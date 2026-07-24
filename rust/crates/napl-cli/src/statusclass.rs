//! Per-prompt status classification (the I/O counterpart of `status-core.ts`).

use std::path::Path;

use napl_core::hash::content_hash;
use napl_core::schemas::{parse_frontmatter, NaplMap, PromptRecord};

use crate::error::CliResult;
use crate::fsutil;

/// The status of a prompt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileStatus {
    /// Locked, attributed, and prompt matches its last gen.
    Clean,
    /// Not generated, or the prompt changed since gen.
    PromptStale,
    /// A generated file was edited or deleted.
    Drift,
    /// Generated files exist but attribution failed.
    Unattributed,
}

impl FileStatus {
    fn label(self) -> &'static str {
        match self {
            FileStatus::Clean => "clean",
            FileStatus::PromptStale => "prompt-stale",
            FileStatus::Drift => "DRIFT",
            FileStatus::Unattributed => "unattributed",
        }
    }
}

/// A classified status entry.
pub struct StatusEntry {
    /// The prompt path (relative to root).
    pub file: String,
    /// The status.
    pub status: FileStatus,
    /// The human-facing detail.
    pub detail: String,
}

impl StatusEntry {
    /// Render the status line exactly as the CLI prints it.
    #[must_use]
    pub fn line(&self) -> String {
        let suffix = if self.detail.is_empty() {
            String::new()
        } else {
            format!(" ({})", self.detail)
        };
        format!("{:<12} {}{}", self.status.label(), self.file, suffix)
    }

    /// Whether this status is an error (fails the CI gate).
    #[must_use]
    pub fn is_error(&self) -> bool {
        matches!(self.status, FileStatus::Drift | FileStatus::Unattributed)
    }
}

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
    let record = map.prompts.get(rel_path);

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
