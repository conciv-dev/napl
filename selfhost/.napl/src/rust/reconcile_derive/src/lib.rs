//! Pure derivation core for the reconcile command: given already-detected
//! drifted files, decides which are eligible to fold back into the prompt and
//! builds the per-file inputs the reconcile task needs.

use drift::{DriftReason, DriftedFile};
use prompts::ReconcileFile;
use text_diff::unified_diff;

fn is_editable(file: &DriftedFile) -> bool {
    matches!(file.reason, DriftReason::Edited) && file.current.is_some()
}

/// Return the drifted files that can be folded back into the prompt: edited
/// files whose current content is available, in their original order.
pub fn editable_drifted(files: &[DriftedFile]) -> Vec<&DriftedFile> {
    files.iter().filter(|f| is_editable(f)).collect()
}

/// Build the reconcile inputs for each editable drifted file, in order.
pub fn build_reconcile_files(files: &[DriftedFile]) -> Vec<ReconcileFile> {
    editable_drifted(files)
        .into_iter()
        .map(|f| ReconcileFile {
            file: f.file.clone(),
            diff: f
                .diff
                .clone()
                .unwrap_or_else(|| unified_diff("", f.current.as_deref().unwrap_or(""))),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn edited(file: &str, current: Option<&str>, diff: Option<&str>) -> DriftedFile {
        DriftedFile {
            file: file.to_string(),
            reason: DriftReason::Edited,
            expected_hash: None,
            actual_hash: None,
            baseline: None,
            current: current.map(str::to_string),
            diff: diff.map(str::to_string),
        }
    }

    fn missing(file: &str) -> DriftedFile {
        DriftedFile {
            file: file.to_string(),
            reason: DriftReason::Missing,
            expected_hash: None,
            actual_hash: None,
            baseline: None,
            current: None,
            diff: None,
        }
    }

    #[test]
    fn editable_drifted_keeps_edited_with_current_in_order() {
        let files = vec![
            edited("a.ts", Some("a content"), None),
            missing("b.ts"),
            edited("c.ts", None, None),
            edited("d.ts", Some("d content"), None),
        ];

        let result = editable_drifted(&files);

        assert_eq!(
            result.iter().map(|f| f.file.as_str()).collect::<Vec<_>>(),
            vec!["a.ts", "d.ts"]
        );
    }

    #[test]
    fn editable_drifted_empty_when_none_qualify() {
        let files = vec![missing("a.ts"), edited("b.ts", None, None)];

        assert!(editable_drifted(&files).is_empty());
    }

    #[test]
    fn build_reconcile_files_uses_recorded_diff_verbatim() {
        let files = vec![edited("a.ts", Some("a content"), Some("PRERECORDED DIFF"))];

        let result = build_reconcile_files(&files);

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].file, "a.ts");
        assert_eq!(result[0].diff, "PRERECORDED DIFF");
    }

    #[test]
    fn build_reconcile_files_computes_diff_when_missing() {
        let files = vec![edited("a.ts", Some("new content"), None)];

        let result = build_reconcile_files(&files);

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].diff, unified_diff("", "new content"));
    }

    #[test]
    fn build_reconcile_files_skips_missing_and_contentless_files() {
        let files = vec![missing("a.ts"), edited("b.ts", None, None)];

        assert!(build_reconcile_files(&files).is_empty());
    }

    #[test]
    fn build_reconcile_files_preserves_order() {
        let files = vec![
            edited("a.ts", Some("a"), Some("diff a")),
            missing("skip.ts"),
            edited("b.ts", Some("b"), Some("diff b")),
        ];

        let result = build_reconcile_files(&files);

        assert_eq!(
            result.iter().map(|f| f.file.as_str()).collect::<Vec<_>>(),
            vec!["a.ts", "b.ts"]
        );
    }
}
