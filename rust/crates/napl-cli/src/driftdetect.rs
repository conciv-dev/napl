//! Generation-time drift detection (the I/O counterpart of `drift.ts`); the
//! report formatting lives in `napl_core::drift`.
//!
//! Stage1: the pure journal-patch replay (`reconstruct_file_content`) is the
//! NAPL-generated `driftdetect_replay` crate (composed on the generated
//! `schemas_journal` + `text_diff` crates), re-exported here; this shell reads
//! the current file off disk and diffs it against the reconstructed baseline.
//! The unit corpus below rides along as the regression net.

use std::collections::BTreeMap;
use std::path::Path;

use napl_core::drift::{DriftReason, DriftedFile, ModuleDrift};
use napl_core::hash::content_hash;
use napl_core::schemas::{JournalEntry, NaplMap};
use napl_core::text_diff::unified_diff;

use crate::error::CliResult;
use crate::fsutil;

pub use driftdetect_replay::reconstruct_file_content;

fn classify_file(
    root: &Path,
    file_path: &str,
    map: &NaplMap,
    journal: &[JournalEntry],
) -> CliResult<Option<DriftedFile>> {
    let abs = root.join(file_path);
    let expected_hash = map.files.get(file_path).map(|f| f.hash.clone());
    let baseline = reconstruct_file_content(journal, file_path);
    if !fsutil::exists(&abs) {
        return Ok(Some(DriftedFile {
            file: file_path.to_string(),
            reason: DriftReason::Missing,
            expected_hash,
            actual_hash: None,
            baseline,
            current: None,
            diff: None,
        }));
    }
    let current = std::fs::read_to_string(&abs)?;
    let actual_hash = content_hash(&current);
    if expected_hash.as_deref() == Some(actual_hash.as_str()) {
        return Ok(None);
    }
    let diff = baseline
        .as_ref()
        .map(|baseline| unified_diff(baseline, &current));
    Ok(Some(DriftedFile {
        file: file_path.to_string(),
        reason: DriftReason::Edited,
        expected_hash,
        actual_hash: Some(actual_hash),
        baseline,
        current: Some(current),
        diff,
    }))
}

/// Detect drifted, attributed files for a target, mirroring `detectGenDrift`.
pub fn detect_gen_drift(
    root: &Path,
    target: &str,
    map: &NaplMap,
    journal: &[JournalEntry],
    module_scope: Option<&str>,
    prompt_paths: &BTreeMap<String, String>,
) -> CliResult<Vec<ModuleDrift>> {
    let mut drifts = Vec::new();
    for (_, record) in map.prompts.iter() {
        if let Some(scope) = module_scope {
            if record.module != scope {
                continue;
            }
        }
        let Some(target_record) = record.targets.get(target) else {
            continue;
        };
        if target_record.unattributed == Some(true) {
            continue;
        }
        let mut files = Vec::new();
        for file_path in &target_record.files {
            if let Some(drifted) = classify_file(root, file_path, map, journal)? {
                files.push(drifted);
            }
        }
        if !files.is_empty() {
            drifts.push(ModuleDrift {
                module: record.module.clone(),
                prompt_file: prompt_paths
                    .get(&record.module)
                    .cloned()
                    .unwrap_or_else(|| record.module.clone()),
                target: target.to_string(),
                files,
            });
        }
    }
    Ok(drifts)
}

#[cfg(test)]
mod tests {
    use super::*;
    use napl_core::schemas::{JournalFile, JournalMode};

    fn entry(gen: i64, patch: &str) -> JournalEntry {
        JournalEntry {
            gen,
            timestamp: format!("t{gen}"),
            module: "greeting".to_string(),
            target: "typescript".to_string(),
            prompt_hash: format!("h{gen}"),
            prompt_diff: String::new(),
            mode: JournalMode::Full,
            files: vec![JournalFile {
                path: ".napl/src/typescript/greet.ts".to_string(),
                patch: patch.to_string(),
                hash_before: None,
                hash_after: "x".to_string(),
            }],
        }
    }

    #[test]
    fn reconstruct_replays_patches_in_order() {
        let entries = vec![
            entry(1, "@@ -0,0 +1,1 @@\n+line one"),
            entry(2, "@@ -1,1 +1,2 @@\n line one\n+line two"),
        ];
        let content = reconstruct_file_content(&entries, ".napl/src/typescript/greet.ts").unwrap();
        assert_eq!(content, "line one\nline two");
    }

    #[test]
    fn reconstruct_returns_none_for_unknown_file() {
        let entries = vec![entry(1, "@@ -0,0 +1,1 @@\n+x")];
        assert!(reconstruct_file_content(&entries, "other.ts").is_none());
    }
}
