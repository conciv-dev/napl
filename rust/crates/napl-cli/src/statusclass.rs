//! Per-prompt status classification (the I/O counterpart of `status-core.ts`).
//!
//! Stage1: the pure status enum and one-line rendering (`FileStatus`,
//! `StatusEntry`, `line`, `is_error`) are the NAPL-generated
//! `statusclass_render` crate, re-exported here; this shell reads generated
//! files off disk to classify each prompt into a `StatusEntry`. The unit corpus
//! below rides along as the regression net.

use std::path::Path;

use napl_core::hash::content_hash;
use napl_core::schemas::{parse_frontmatter, NaplMap, PromptRecord};

use crate::error::CliResult;
use crate::fsutil;

pub use statusclass_render::{FileStatus, StatusEntry};

struct DriftResult {
    detail: Option<String>,
}

fn detect_drift(root: &Path, record: &PromptRecord, map: &NaplMap) -> CliResult<DriftResult> {
    for (target, target_record) in record.targets.iter() {
        if target_record.unattributed == Some(true) {
            continue;
        }
        for file_path in &target_record.files {
            let abs = root.join(file_path);
            if !fsutil::exists(&abs) {
                return Ok(DriftResult {
                    detail: Some(format!("{target}: {file_path} is missing")),
                });
            }
            let expected = map.files.get(file_path).map(|f| &f.hash);
            let actual = content_hash(&std::fs::read_to_string(&abs)?);
            if expected != Some(&actual) {
                return Ok(DriftResult {
                    detail: Some(format!("{target}: {file_path} was edited")),
                });
            }
        }
    }
    Ok(DriftResult { detail: None })
}

fn detect_unattributed(record: &PromptRecord) -> Option<String> {
    for (target, target_record) in record.targets.iter() {
        if target_record.unattributed == Some(true) {
            return Some(format!(
                "generated files lack prompt attribution — run napl gen {target} --force"
            ));
        }
    }
    None
}

fn detect_prompt_stale(
    record: &PromptRecord,
    declared_targets: &[String],
    prompt_hash: &str,
) -> Option<String> {
    for target in declared_targets {
        match record.targets.get(target) {
            None => return Some(format!("{target}: not generated")),
            Some(target_record) => {
                if target_record.prompt_hash_at_gen.as_deref() != Some(prompt_hash) {
                    return Some("prompt changed since gen".to_string());
                }
            }
        }
    }
    None
}

/// Classify one prompt, mirroring `classifyPrompt`.
pub fn classify_prompt(
    root: &Path,
    rel_path: &str,
    raw: &str,
    map: &NaplMap,
) -> CliResult<StatusEntry> {
    let parsed = parse_frontmatter(raw)?;
    let frontmatter = parsed.frontmatter;
    let prompt_hash = content_hash(raw);
    let record = map.prompts.get(&frontmatter.module);

    if let Some(record) = record {
        let drift = detect_drift(root, record, map)?;
        if let Some(detail) = drift.detail {
            return Ok(StatusEntry {
                file: rel_path.to_string(),
                status: FileStatus::Drift,
                detail,
            });
        }
        if let Some(detail) = detect_unattributed(record) {
            return Ok(StatusEntry {
                file: rel_path.to_string(),
                status: FileStatus::Unattributed,
                detail,
            });
        }
    }

    let Some(record) = record else {
        return Ok(StatusEntry {
            file: rel_path.to_string(),
            status: FileStatus::PromptStale,
            detail: "never generated".to_string(),
        });
    };

    if let Some(detail) = detect_prompt_stale(record, &frontmatter.targets, &prompt_hash) {
        return Ok(StatusEntry {
            file: rel_path.to_string(),
            status: FileStatus::PromptStale,
            detail,
        });
    }

    Ok(StatusEntry {
        file: rel_path.to_string(),
        status: FileStatus::Clean,
        detail: String::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(status: FileStatus, detail: &str) -> StatusEntry {
        StatusEntry {
            file: "examples/greeting.napl".to_string(),
            status,
            detail: detail.to_string(),
        }
    }

    #[test]
    fn line_pads_status_to_twelve() {
        assert_eq!(
            entry(FileStatus::Clean, "").line(),
            "clean        examples/greeting.napl"
        );
        assert_eq!(
            entry(FileStatus::PromptStale, "never generated").line(),
            "prompt-stale examples/greeting.napl (never generated)"
        );
        assert_eq!(
            entry(FileStatus::Drift, "typescript: x was edited").line(),
            "DRIFT        examples/greeting.napl (typescript: x was edited)"
        );
        assert_eq!(
            entry(FileStatus::Unattributed, "run napl gen typescript --force").line(),
            "unattributed examples/greeting.napl (run napl gen typescript --force)"
        );
    }

    #[test]
    fn error_statuses_flagged() {
        assert!(entry(FileStatus::Drift, "").is_error());
        assert!(entry(FileStatus::Unattributed, "").is_error());
        assert!(!entry(FileStatus::Clean, "").is_error());
        assert!(!entry(FileStatus::PromptStale, "").is_error());
    }
}
