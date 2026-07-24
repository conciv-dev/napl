//! Generated-file healing: git-style move detection. A generated file the map
//! tracks can vanish from its recorded path because a hand `mv` (or a rename in
//! an editor) relocated it. Rather than reporting a false "deleted +
//! unattributed", the toolchain heals: it finds the untracked file that carries
//! the moved content, rewrites the path in the map, journals a `move` entry, and
//! relocks it at the new path. An exact content-hash match is a clean heal
//! (status stays clean); a line-similar match is a moved-and-drifted file (the
//! path is healed, then the normal drift machinery reports the content change).
//! Two candidates with the same hash are ambiguous — a hard error, never a
//! guess.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use napl_core::hash::content_hash;
use napl_core::schemas::{JournalEntry, JournalFile, JournalMode, NaplMap};
use napl_core::targets::get_adapter;
use napl_core::text_diff::to_lines;

use crate::clock::now;
use crate::driftdetect::reconstruct_file_content;
use crate::error::{CliError, CliResult};
use crate::fsutil::{self, READONLY_MODE};
use crate::paths::{rel_to, NaplPaths};
use crate::snapshot::{make_filter, snapshot_contents, snapshot_hashes};
use crate::state::append_journal_entry;

/// A file the map tracked at `old_path`, healed to `new_path`. `drifted` is true
/// when the content also changed (matched by line similarity, not hash).
pub struct HealedMove {
    pub old_path: String,
    pub new_path: String,
    pub target: String,
    pub drifted: bool,
}

fn lcs_len(a: &[String], b: &[String]) -> usize {
    if a.is_empty() || b.is_empty() {
        return 0;
    }
    let mut prev = vec![0usize; b.len() + 1];
    let mut curr = vec![0usize; b.len() + 1];
    for ai in a {
        for (j, bj) in b.iter().enumerate() {
            curr[j + 1] = if ai == bj {
                prev[j] + 1
            } else {
                curr[j].max(prev[j + 1])
            };
        }
        std::mem::swap(&mut prev, &mut curr);
        curr.fill(0);
    }
    prev[b.len()]
}

/// Line-based similarity of at least 50%: the longest common subsequence of
/// lines covers at least half of the longer file. Kept as integer arithmetic so
/// the threshold is exact — `2 * common >= max(len_a, len_b)`.
fn is_line_similar(before: &str, current: &str) -> bool {
    let a = to_lines(before);
    let b = to_lines(current);
    let denom = a.len().max(b.len());
    if denom == 0 {
        return true;
    }
    lcs_len(&a, &b) * 2 >= denom
}

struct Untracked {
    rel: String,
    hash: String,
    content: String,
}

fn collect_untracked(
    root: &Path,
    paths: &NaplPaths,
    targets: &BTreeSet<String>,
    tracked: &BTreeSet<String>,
) -> CliResult<BTreeMap<String, Vec<Untracked>>> {
    let mut by_target: BTreeMap<String, Vec<Untracked>> = BTreeMap::new();
    for target in targets {
        let Ok(adapter) = get_adapter(target) else {
            continue;
        };
        let target_dir = paths.src_dir.join(target);
        if !target_dir.exists() {
            continue;
        }
        let filter = make_filter(
            &adapter.attribution_exclude_dirs,
            &adapter.attribution_exclude_files,
            &adapter.attribution_exclude_root_files,
            &adapter.attribution_exclude_suffixes,
        );
        let hashes = snapshot_hashes(&target_dir, &filter)?;
        let contents = snapshot_contents(&target_dir, &filter)?;
        let mut untracked = Vec::new();
        for (abs, hash) in &hashes {
            let rel = rel_to(root, Path::new(abs));
            if tracked.contains(&rel) {
                continue;
            }
            let content = contents.get(abs).cloned().unwrap_or_default();
            untracked.push(Untracked {
                rel,
                hash: hash.clone(),
                content,
            });
        }
        by_target.insert(target.clone(), untracked);
    }
    Ok(by_target)
}

fn rename_file_record(map: &mut NaplMap, old: &str, new: &str) {
    if let Some(record) = map.files.remove(old) {
        map.files.insert(new.to_string(), record);
    }
    let prompt_keys: Vec<String> = map.prompts.keys().cloned().collect();
    for prompt_key in prompt_keys {
        let Some(prompt) = map.prompts.get_mut(&prompt_key) else {
            continue;
        };
        let target_keys: Vec<String> = prompt.targets.keys().cloned().collect();
        for target_key in target_keys {
            if let Some(target_record) = prompt.targets.get_mut(&target_key) {
                for file in &mut target_record.files {
                    if file == old {
                        *file = new.to_string();
                    }
                }
            }
        }
    }
}

/// Heal every tracked file the map lost to a move, mutating `map` and appending
/// one `move` journal entry per heal. Returns the heals applied (empty when
/// nothing moved). The caller persists the mutated map.
pub fn heal_moved_files(
    root: &Path,
    paths: &NaplPaths,
    map: &mut NaplMap,
    journal: &[JournalEntry],
) -> CliResult<Vec<HealedMove>> {
    let tracked: BTreeSet<String> = map.files.iter().map(|(k, _)| k.clone()).collect();
    let missing: Vec<String> = tracked
        .iter()
        .filter(|rel| !fsutil::exists(&root.join(rel)))
        .cloned()
        .collect();
    if missing.is_empty() {
        return Ok(Vec::new());
    }

    let targets: BTreeSet<String> = map.files.iter().map(|(_, r)| r.target.clone()).collect();
    let untracked = collect_untracked(root, paths, &targets, &tracked)?;

    let mut heals: Vec<HealedMove> = Vec::new();
    let mut claimed: BTreeSet<String> = BTreeSet::new();

    for old in &missing {
        let Some(record) = map.files.get(old) else {
            continue;
        };
        let target = record.target.clone();
        let old_hash = record.hash.clone();
        let empty: Vec<Untracked> = Vec::new();
        let pool = untracked.get(&target).unwrap_or(&empty);

        let exact: Vec<&Untracked> = pool
            .iter()
            .filter(|c| c.hash == old_hash && !claimed.contains(&c.rel))
            .collect();
        if exact.len() > 1 {
            return Err(CliError::new(format!(
                "cannot heal moved file '{old}' ({target}): {} untracked files have identical content ({}). Rename or remove the duplicate so the move is unambiguous.",
                exact.len(),
                exact.iter().map(|c| c.rel.as_str()).collect::<Vec<_>>().join(", ")
            )));
        }
        if let Some(candidate) = exact.first() {
            claimed.insert(candidate.rel.clone());
            heals.push(HealedMove {
                old_path: old.clone(),
                new_path: candidate.rel.clone(),
                target,
                drifted: false,
            });
            continue;
        }

        let baseline = reconstruct_file_content(journal, old).unwrap_or_default();
        let similar: Vec<&Untracked> = pool
            .iter()
            .filter(|c| {
                !claimed.contains(&c.rel) && is_line_similar(&baseline, &c.content)
            })
            .collect();
        if similar.len() > 1 {
            return Err(CliError::new(format!(
                "cannot heal moved file '{old}' ({target}): {} untracked files are similar candidates ({}). Rename or remove the duplicate so the move is unambiguous.",
                similar.len(),
                similar.iter().map(|c| c.rel.as_str()).collect::<Vec<_>>().join(", ")
            )));
        }
        if let Some(candidate) = similar.first() {
            claimed.insert(candidate.rel.clone());
            heals.push(HealedMove {
                old_path: old.clone(),
                new_path: candidate.rel.clone(),
                target,
                drifted: true,
            });
        }
    }

    if heals.is_empty() {
        return Ok(heals);
    }

    let mut next_gen = napl_core::schemas::next_gen_number(journal);
    for heal in &heals {
        let (module, hash_after) = {
            let record = map.files.get(&heal.old_path);
            let module = record
                .and_then(|r| r.prompts.first().cloned())
                .unwrap_or_else(|| heal.new_path.clone());
            let hash_before = record.map(|r| r.hash.clone());
            let new_abs = root.join(&heal.new_path);
            let hash_after = content_hash(&std::fs::read_to_string(&new_abs)?);
            (module, (hash_before, hash_after))
        };
        let (hash_before, hash_after) = hash_after;
        rename_file_record(map, &heal.old_path, &heal.new_path);
        if !heal.drifted {
            let new_abs = root.join(&heal.new_path);
            fsutil::set_mode(&new_abs, READONLY_MODE)?;
        }
        let entry = JournalEntry {
            gen: next_gen,
            timestamp: now(),
            module,
            target: heal.target.clone(),
            prompt_hash: String::new(),
            prompt_diff: String::new(),
            mode: JournalMode::Move,
            files: vec![JournalFile {
                path: heal.new_path.clone(),
                patch: format!("moved {} -> {}", heal.old_path, heal.new_path),
                hash_before,
                hash_after,
            }],
        };
        append_journal_entry(&paths.journal_path, &entry)?;
        next_gen += 1;
    }

    Ok(heals)
}
