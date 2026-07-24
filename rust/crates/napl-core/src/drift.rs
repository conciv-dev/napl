//! Drift model and the guided drift-report formatter. The detection I/O lives
//! in the CLI; this module owns the pure types and the pinned report text.

/// Why a file is considered drifted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DriftReason {
    /// The file was edited by hand.
    Edited,
    /// The locked file was deleted.
    Missing,
}

/// A single drifted file within a module.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DriftedFile {
    /// The file path relative to the project root.
    pub file: String,
    /// Why it is drifted.
    pub reason: DriftReason,
    /// The hash recorded in the map, if known.
    pub expected_hash: Option<String>,
    /// The hash of the current on-disk content, if present.
    pub actual_hash: Option<String>,
    /// The journal-reconstructed baseline content, if recoverable.
    pub baseline: Option<String>,
    /// The current on-disk content, if present.
    pub current: Option<String>,
    /// The baseline-to-current unified diff, if computable.
    pub diff: Option<String>,
}

/// A module with one or more drifted files.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleDrift {
    /// The module name.
    pub module: String,
    /// The prompt file that owns the module.
    pub prompt_file: String,
    /// The target.
    pub target: String,
    /// The drifted files.
    pub files: Vec<DriftedFile>,
}

fn indent(text: &str, pad: &str) -> String {
    text.split('\n')
        .map(|line| format!("{pad}{line}"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn format_file(file: &DriftedFile) -> String {
    let mut lines: Vec<String> = Vec::new();
    if file.reason == DriftReason::Missing {
        lines.push(format!(
            "    {} (missing — the locked file was deleted)",
            file.file
        ));
        if let Some(hash) = &file.expected_hash {
            lines.push(format!("      recorded hash: {hash}"));
        }
        return lines.join("\n");
    }
    lines.push(format!("    {} (edited by hand)", file.file));
    match &file.diff {
        Some(diff) if !diff.trim().is_empty() => {
            lines.push("      recorded baseline -> current:".to_string());
            lines.push(indent(diff, "      "));
        }
        _ => {
            lines.push("      baseline content is not recoverable from the journal (pre-journal state); comparing hashes only:".to_string());
            lines.push(format!(
                "      recorded: {}",
                file.expected_hash.as_deref().unwrap_or("(none)")
            ));
            lines.push(format!(
                "      current:  {}",
                file.actual_hash.as_deref().unwrap_or("(none)")
            ));
        }
    }
    lines.join("\n")
}

/// Format the guided drift report, mirroring `formatGenDriftReport`.
#[must_use]
pub fn format_gen_drift_report(drifts: &[ModuleDrift], target: &str) -> String {
    let mut lines: Vec<String> = Vec::new();
    lines.push(format!(
        "BLOCKED  drift detected — cannot run 'napl gen {target}' while generated files have hand edits that are not reflected in any prompt."
    ));
    lines.push(String::new());
    for drift in drifts {
        let count = drift.files.len();
        lines.push(format!(
            "  module {} ({}) — {count} file(s) drifted (from {}):",
            drift.module, drift.target, drift.prompt_file
        ));
        for file in &drift.files {
            lines.push(format_file(file));
        }
        lines.push(String::new());
        lines.push("  Resolve it one of three ways:".to_string());
        lines.push(format!(
            "    1) napl reconcile {}  — fold this edit back into your prompt (coming soon)",
            drift.module
        ));
        lines.push(format!(
            "    2) napl gen {target} --module {} --force  — discard the edit, the prompt wins",
            drift.module
        ));
        lines.push(format!(
            "    3) edit the prompt to describe the change, then napl gen {target}"
        ));
        lines.push(String::new());
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn edited(diff: Option<&str>) -> ModuleDrift {
        ModuleDrift {
            module: "greeting".to_string(),
            prompt_file: "examples/greeting.napl".to_string(),
            target: "typescript".to_string(),
            files: vec![DriftedFile {
                file: ".napl/src/typescript/greet.ts".to_string(),
                reason: DriftReason::Edited,
                expected_hash: Some("aaa".to_string()),
                actual_hash: Some("bbb".to_string()),
                baseline: Some("old\n".to_string()),
                current: Some("new\n".to_string()),
                diff: diff.map(ToString::to_string),
            }],
        }
    }

    #[test]
    fn report_lists_module_file_and_three_resolutions() {
        let report =
            format_gen_drift_report(&[edited(Some("@@ -1,1 +1,1 @@\n-old\n+new"))], "typescript");
        assert!(report.contains("drift detected"));
        assert!(report.contains("module greeting (typescript)"));
        assert!(report.contains(".napl/src/typescript/greet.ts (edited by hand)"));
        assert!(report.contains("recorded baseline -> current:"));
        assert!(report.contains("1) napl reconcile greeting"));
        assert!(report.contains("2) napl gen typescript --module greeting --force"));
        assert!(
            report.contains("3) edit the prompt to describe the change, then napl gen typescript")
        );
    }

    #[test]
    fn report_falls_back_to_hashes_without_diff() {
        let report = format_gen_drift_report(&[edited(None)], "typescript");
        assert!(report.contains("comparing hashes only"));
        assert!(report.contains("recorded: aaa"));
        assert!(report.contains("current:  bbb"));
    }

    #[test]
    fn report_marks_missing_file() {
        let drift = ModuleDrift {
            module: "m".to_string(),
            prompt_file: "p.napl".to_string(),
            target: "typescript".to_string(),
            files: vec![DriftedFile {
                file: "gone.ts".to_string(),
                reason: DriftReason::Missing,
                expected_hash: Some("h".to_string()),
                actual_hash: None,
                baseline: None,
                current: None,
                diff: None,
            }],
        };
        let report = format_gen_drift_report(&[drift], "typescript");
        assert!(report.contains("gone.ts (missing — the locked file was deleted)"));
        assert!(report.contains("recorded hash: h"));
    }
}
