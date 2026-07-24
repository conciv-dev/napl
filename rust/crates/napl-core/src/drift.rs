//! Stage1 adapter over the NAPL-generated `drift` crate.

pub use gen_drift::{format_gen_drift_report, DriftReason, DriftedFile, ModuleDrift};

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
