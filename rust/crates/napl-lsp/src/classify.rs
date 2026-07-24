//! Read-only prompt status classification for diagnostics. This mirrors the
//! CLI's `status` classifier but returns a value instead of printing, so the
//! server can turn DRIFT / prompt-stale into squiggles without depending on the
//! CLI crate.

use std::path::Path;

use napl_core::hash::content_hash;
use napl_core::schemas::{parse_frontmatter, NaplMap, PromptRecord, SchemaError};

/// The classification of a single prompt file.
pub enum PromptStatus {
    Clean,
    PromptStale { detail: String },
    Drift { target: String, file: String },
    Unattributed,
}

fn detect_drift(root: &Path, record: &PromptRecord, map: &NaplMap) -> Option<PromptStatus> {
    for (target, target_record) in record.targets.iter() {
        if target_record.unattributed == Some(true) {
            continue;
        }
        for file_path in &target_record.files {
            let abs = root.join(file_path);
            if !abs.exists() {
                return Some(PromptStatus::Drift {
                    target: target.clone(),
                    file: file_path.clone(),
                });
            }
            let actual = content_hash(&std::fs::read_to_string(&abs).unwrap_or_default());
            let expected = map.files.get(file_path).map(|f| &f.hash);
            if expected != Some(&actual) {
                return Some(PromptStatus::Drift {
                    target: target.clone(),
                    file: file_path.clone(),
                });
            }
        }
    }
    None
}

fn detect_unattributed(record: &PromptRecord) -> Option<PromptStatus> {
    for (_target, target_record) in record.targets.iter() {
        if target_record.unattributed == Some(true) {
            return Some(PromptStatus::Unattributed);
        }
    }
    None
}

fn detect_prompt_stale(
    record: &PromptRecord,
    declared_targets: &[String],
    prompt_hash: &str,
) -> Option<PromptStatus> {
    for target in declared_targets {
        match record.targets.get(target) {
            None => {
                return Some(PromptStatus::PromptStale {
                    detail: format!("{target}: not generated"),
                })
            }
            Some(target_record) => {
                if target_record.prompt_hash_at_gen.as_deref() != Some(prompt_hash) {
                    return Some(PromptStatus::PromptStale {
                        detail: "prompt changed since gen".to_string(),
                    });
                }
            }
        }
    }
    None
}

/// Classify one prompt, or return the frontmatter error message on `Err`.
pub fn classify_prompt(
    root: &Path,
    rel_path: &str,
    raw: &str,
    map: &NaplMap,
) -> Result<PromptStatus, String> {
    let parsed = parse_frontmatter(raw).map_err(|error| match error {
        SchemaError::Deserialize(message) | SchemaError::Validation(message) => message,
    })?;
    let prompt_hash = content_hash(raw);
    let record = map.prompts.get(rel_path);

    if let Some(record) = record {
        if let Some(drift) = detect_drift(root, record, map) {
            return Ok(drift);
        }
        if let Some(unattributed) = detect_unattributed(record) {
            return Ok(unattributed);
        }
    }

    let Some(record) = record else {
        return Ok(PromptStatus::PromptStale {
            detail: "never generated".to_string(),
        });
    };

    if let Some(stale) = detect_prompt_stale(record, &parsed.frontmatter.targets, &prompt_hash) {
        return Ok(stale);
    }

    Ok(PromptStatus::Clean)
}
