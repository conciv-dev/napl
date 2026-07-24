//! Equivalence gate for the `cmd_reconcile` module's pure derivation slice.
//!
//! Replays the EXACT unit-test corpus of the hand-written `napl-cli`
//! `cmd_reconcile` module (rust/crates/napl-cli/src/cmd_reconcile.rs — the
//! `tests` module) against the NAPL-generated `reconcile_derive` crate, which
//! composes on the generated `drift`, `prompts`, and `text_diff` crates by path.

use drift::{DriftReason, DriftedFile};
use reconcile_derive::{build_reconcile_files, editable_drifted};
use text_diff::unified_diff;

fn drifted(
    file: &str,
    reason: DriftReason,
    current: Option<&str>,
    diff: Option<&str>,
) -> DriftedFile {
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
    let files = vec![drifted(
        "a.ts",
        DriftReason::Edited,
        Some("hand edit"),
        Some("PRERECORDED DIFF"),
    )];
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
