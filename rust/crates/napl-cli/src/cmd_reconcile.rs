//! `napl reconcile <target>`: fold drifted hand edits back into the prompt. For
//! each drifted file it runs the configured agent with a reconcile task that
//! amends the prompt so a future regeneration reproduces the edited behavior,
//! then accepts the edited source as the new baseline, journals a reconcile
//! entry, and clears the drift (leaving the module stale for the next gen).
//!
//! Stage1: the pure derivation slice (`editable_drifted`, `build_reconcile_files`)
//! is the NAPL-generated `reconcile_derive` crate, re-exported here; this shell
//! keeps the drift detection, the agent run, and the journal/map writes. The unit
//! corpus below rides along as the regression net.

use std::path::Path;

use napl_core::drift::ModuleDrift;
use napl_core::hash::content_hash;
use napl_core::incremental::diff_body_lines;
use napl_core::prompts::build_reconcile_task;
use napl_core::schemas::{parse_frontmatter, JournalEntry, JournalFile, JournalMode, NaplMap};
use napl_core::targets::get_adapter;
use napl_core::text_diff::unified_diff;

use reconcile_derive::{build_reconcile_files, editable_drifted};

use crate::clock::now;
use crate::driftdetect::detect_gen_drift;
use crate::error::{CliError, CliResult};
use crate::fsutil::{self, READONLY_MODE};
use crate::paths::{rel_to, resolve_paths, NaplPaths};
use crate::process::{acquire_gen_lock, require_engine, resolve_engine, run_coding_agent};
use crate::state::{append_journal_entry, read_journal, read_lock, read_map, write_map};

/// Arguments for the reconcile command.
pub struct ReconcileArgs<'a> {
    /// The target language.
    pub target: &'a str,
    /// Scope to a single module by name.
    pub module: Option<&'a str>,
}

fn to_posix(path: &str) -> String {
    path.replace(std::path::MAIN_SEPARATOR, "/")
}

/// Run reconcile: detect drift, amend prompts, accept edits, clear drift.
pub fn run(root: &Path, args: &ReconcileArgs) -> CliResult<i32> {
    let paths = resolve_paths(root);
    let lock = read_lock(&paths.lock_path)?;
    if matches!(lock.backend, napl_core::schemas::Backend::AnthropicApi) {
        return Err(CliError::new(
            "the anthropic-api backend is not yet supported in the Rust CLI — use the TS CLI or set backend to \"claude-cli\" in .napl/lock.json.",
        ));
    }
    let adapter = get_adapter(args.target).map_err(CliError::new)?;
    let model = lock.model.clone();
    let engine = resolve_engine(&napl_core::schemas::resolve_agent_config(&lock));
    require_engine(&engine)?;

    let mut map = read_map(&paths.map_path)?;
    let (journal, journal_warnings) = read_journal(&paths.journal_path)?;
    for warning in &journal_warnings {
        println!("{warning}");
    }
    let drifts = detect_gen_drift(root, args.target, &map, &journal, args.module)?;
    if drifts.is_empty() {
        println!(
            "nothing to reconcile — no drifted files for target '{}'.",
            args.target
        );
        return Ok(0);
    }

    let genlock = acquire_gen_lock(&paths.gen_lock_path)?;
    let result = run_reconcile_locked(
        root,
        &paths,
        args,
        &model,
        &engine,
        &adapter.agent_tools,
        &journal,
        &drifts,
        &mut map,
    );
    genlock.release();
    let summary = result?;
    println!(
        "reconciled {} module(s), {} file(s)",
        summary.modules, summary.files
    );
    Ok(0)
}

struct ReconcileSummary {
    modules: usize,
    files: usize,
}

#[allow(clippy::too_many_arguments)]
fn run_reconcile_locked(
    root: &Path,
    paths: &NaplPaths,
    args: &ReconcileArgs,
    model: &str,
    engine: &crate::process::AgentEngine,
    agent_tools: &[String],
    journal: &[JournalEntry],
    drifts: &[ModuleDrift],
    map: &mut NaplMap,
) -> CliResult<ReconcileSummary> {
    let target = args.target;
    let mut next_gen = napl_core::schemas::next_gen_number(journal);
    let mut reconciled_modules = 0;
    let mut reconciled_files = 0;

    for drift in drifts {
        let editable = editable_drifted(&drift.files);
        let missing = drift.files.len() - editable.len();
        if missing > 0 {
            println!(
                "  note: {missing} file(s) in module {} were deleted, not edited — skipping them (run 'napl gen {target} --module {} --force' to restore).",
                drift.module, drift.module
            );
        }
        if editable.is_empty() {
            continue;
        }

        println!(
            "reconcile {} ({target}) — {} drifted file(s) from {}",
            drift.module,
            editable.len(),
            drift.prompt_file
        );

        let prompt_abs = root.join(&drift.prompt_file);
        let before_raw = std::fs::read_to_string(&prompt_abs)?;
        let before_body = parse_frontmatter(&before_raw)?.body;

        let reconcile_files = build_reconcile_files(&drift.files);
        let task = build_reconcile_task(
            &drift.module,
            &drift.prompt_file,
            &before_body,
            &reconcile_files,
        );
        println!("  amending prompt {} via the coding agent", drift.prompt_file);
        run_coding_agent(engine, &task, root, model, agent_tools)?;

        let after_raw = std::fs::read_to_string(&prompt_abs)?;
        let after_body = parse_frontmatter(&after_raw)?.body;
        let prompt_hash = content_hash(&after_raw);
        let prompt_diff = if before_body == after_body {
            String::new()
        } else {
            diff_body_lines(&before_body, &after_body).unified
        };

        let mut journal_files: Vec<JournalFile> = Vec::new();
        for file in editable {
            let abs = root.join(&file.file);
            let current = std::fs::read_to_string(&abs)?;
            let current_hash = content_hash(&current);
            let patch = unified_diff(file.baseline.as_deref().unwrap_or(""), &current);
            if let Some(record) = map.files.get(&file.file) {
                let mut updated = record.clone();
                updated.hash.clone_from(&current_hash);
                map.files.insert(file.file.clone(), updated);
            }
            fsutil::set_mode(&abs, READONLY_MODE)?;
            println!(
                "  accepted {} as the new baseline (re-locked 0444)",
                file.file
            );
            journal_files.push(JournalFile {
                path: to_posix(&file.file),
                patch,
                hash_before: file.expected_hash.clone(),
                hash_after: current_hash,
            });
            reconciled_files += 1;
        }

        let entry = JournalEntry {
            gen: next_gen,
            timestamp: now(),
            module: drift.module.clone(),
            target: target.to_string(),
            prompt_hash,
            prompt_diff,
            mode: JournalMode::Reconcile,
            files: journal_files.clone(),
        };
        append_journal_entry(&paths.journal_path, &entry)?;
        println!(
            "  journal: reconcile #{next_gen} recorded ({} file patch(es)) -> {}",
            journal_files.len(),
            rel_to(root, &paths.journal_path)
        );
        println!(
            "  module {} left stale — run 'napl gen {target}' to regenerate from the amended prompt",
            drift.module
        );
        next_gen += 1;
        reconciled_modules += 1;
    }

    write_map(&paths.map_path, map)?;
    Ok(ReconcileSummary {
        modules: reconciled_modules,
        files: reconciled_files,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use napl_core::drift::{DriftReason, DriftedFile};

    fn drifted(file: &str, reason: DriftReason, current: Option<&str>, diff: Option<&str>) -> DriftedFile {
        DriftedFile {
            file: file.to_string(),
            reason,
            expected_hash: None,
            actual_hash: None,
            baseline: None,
            current: current.map(str::to_string),
            diff: diff.map(str::to_string),
        }
    }

    #[test]
    fn editable_drifted_keeps_only_edited_files_with_current_content() {
        let files = vec![
            drifted("a.ts", DriftReason::Edited, Some("a"), None),
            drifted("b.ts", DriftReason::Missing, None, None),
            drifted("c.ts", DriftReason::Edited, None, None),
            drifted("d.ts", DriftReason::Edited, Some("d"), None),
        ];
        let editable = editable_drifted(&files);
        let names: Vec<&str> = editable.iter().map(|f| f.file.as_str()).collect();
        assert_eq!(names, vec!["a.ts", "d.ts"]);
    }

    #[test]
    fn build_reconcile_files_uses_recorded_diff_when_present() {
        let files = vec![drifted("a.ts", DriftReason::Edited, Some("hand edit"), Some("PRERECORDED DIFF"))];
        let built = build_reconcile_files(&files);
        assert_eq!(built.len(), 1);
        assert_eq!(built[0].file, "a.ts");
        assert_eq!(built[0].diff, "PRERECORDED DIFF");
    }

    #[test]
    fn build_reconcile_files_falls_back_to_added_from_empty_diff() {
        let files = vec![drifted("a.ts", DriftReason::Edited, Some("new content"), None)];
        let built = build_reconcile_files(&files);
        assert_eq!(built.len(), 1);
        assert_eq!(built[0].diff, unified_diff("", "new content"));
    }

    #[test]
    fn build_reconcile_files_skips_deleted_and_contentless_files() {
        let files = vec![
            drifted("gone.ts", DriftReason::Missing, None, None),
            drifted("empty.ts", DriftReason::Edited, None, None),
        ];
        assert!(build_reconcile_files(&files).is_empty());
    }
}
