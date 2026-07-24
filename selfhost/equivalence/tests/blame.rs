//! Equivalence gate for the `blame` module.
//!
//! This is the EXACT unit-test corpus of the hand-written `napl-core` `blame`
//! module (rust/crates/napl-core/src/blame.rs), replayed against the
//! NAPL-generated `blame` crate under selfhost/.napl/src/rust/blame/. Each case
//! asserts the same input -> output the hand-written module asserts for itself.
//!
//! The patches are built with the NAPL-generated sibling `text_diff` crate's
//! `unified_diff` — the same generated crate `blame` path-depends on for its hunk
//! parsing and line splitting — so this test also proves the intra-workspace
//! composition `blame -> text_diff`.

use blame::{apply_patch_to_blame, blame_file, blame_line_at, first_prompt_diff_line, BlameSourceEntry};
use text_diff::unified_diff;

fn file_patch(before: Option<&str>, after: &str) -> String {
    unified_diff(before.unwrap_or(""), after)
}

struct Version {
    gen: i64,
    content: &'static str,
    module: &'static str,
    timestamp: String,
}

fn history(versions: &[Version]) -> Vec<BlameSourceEntry> {
    let mut entries = Vec::new();
    let mut prev: Option<&str> = None;
    for v in versions {
        entries.push(BlameSourceEntry {
            gen: v.gen,
            timestamp: v.timestamp.clone(),
            module: v.module.to_string(),
            patch: file_patch(prev, v.content),
            prompt_diff: String::new(),
        });
        prev = Some(v.content);
    }
    entries
}

fn ver(gen: i64, content: &'static str) -> Version {
    Version {
        gen,
        content,
        module: "demo",
        timestamp: format!("2026-07-2{gen}T00:00:00.000Z"),
    }
}

fn gens(entries: &[BlameSourceEntry], content: &str) -> Vec<i64> {
    blame_file(entries, content)
        .into_iter()
        .map(|line| line.gen)
        .collect()
}

#[test]
fn single_gen_file_all_lines_that_gen() {
    let content = "A\nB\nC\n";
    let entries = history(&[ver(1, content)]);
    assert_eq!(gens(&entries, content), vec![1, 1, 1]);
}

#[test]
fn untouched_stays_old_modified_moves() {
    let entries = history(&[ver(1, "A\nB\nC\n"), ver(3, "A\nB2\nC\n")]);
    assert_eq!(gens(&entries, "A\nB2\nC\n"), vec![1, 3, 1]);
}

#[test]
fn appended_line_attributed_to_adding_gen() {
    let entries = history(&[ver(1, "A\n"), ver(2, "A\nB\n")]);
    assert_eq!(gens(&entries, "A\nB\n"), vec![1, 2]);
}

#[test]
fn line_moved_down_by_insertion_above() {
    let entries = history(&[ver(1, "A\nB\n"), ver(2, "X\nA\nB\n")]);
    assert_eq!(gens(&entries, "X\nA\nB\n"), vec![2, 1, 1]);
}

#[test]
fn created_then_edited_across_multiple_hunks() {
    let entries = history(&[ver(1, "a\nb\nc\nd\ne\n"), ver(3, "a\nB\nc\nd\nE\nf\n")]);
    assert_eq!(gens(&entries, "a\nB\nc\nd\nE\nf\n"), vec![1, 3, 1, 1, 3, 3]);
}

#[test]
fn carries_timestamp_and_module_from_attributing_gen() {
    let versions = [
        Version {
            gen: 1,
            content: "A\nB\n",
            module: "first",
            timestamp: "ts1".to_string(),
        },
        Version {
            gen: 4,
            content: "A\nB2\n",
            module: "second",
            timestamp: "ts4".to_string(),
        },
    ];
    let entries = history(&versions);
    let blamed = blame_file(&entries, "A\nB2\n");
    assert_eq!(blamed[0].gen, 1);
    assert_eq!(blamed[0].module, "first");
    assert_eq!(blamed[0].timestamp, "ts1");
    assert_eq!(blamed[0].text, "A");
    assert_eq!(blamed[1].gen, 4);
    assert_eq!(blamed[1].module, "second");
    assert_eq!(blamed[1].timestamp, "ts4");
    assert_eq!(blamed[1].text, "B2");
}

#[test]
fn blame_line_at_returns_single_line() {
    let entries = history(&[ver(1, "A\nB\nC\n"), ver(2, "A\nB2\nC\n")]);
    assert_eq!(
        blame_line_at(&entries, "A\nB2\nC\n", 2).map(|b| b.gen),
        Some(2)
    );
    assert_eq!(
        blame_line_at(&entries, "A\nB2\nC\n", 1).map(|b| b.gen),
        Some(1)
    );
}

#[test]
fn blame_line_at_out_of_range_is_none() {
    let entries = history(&[ver(1, "A\n")]);
    assert!(blame_line_at(&entries, "A\n", 9).is_none());
}

#[test]
fn apply_creation_patch_tags_every_inserted_line() {
    let patch = file_patch(None, "x\ny\n");
    assert_eq!(apply_patch_to_blame(&[], &patch, 7), vec![7, 7]);
}

#[test]
fn empty_patch_is_no_op() {
    assert_eq!(apply_patch_to_blame(&[1, 2, 3], "", 9), vec![1, 2, 3]);
}

#[test]
fn first_prompt_diff_line_prefers_addition() {
    let diff = "@@ -1,2 +1,2 @@\n old\n-was this\n+now that\n";
    assert_eq!(first_prompt_diff_line(diff), "now that");
}

#[test]
fn first_prompt_diff_line_falls_back_to_removal() {
    assert_eq!(
        first_prompt_diff_line("@@ -1,1 +0,0 @@\n-gone now\n"),
        "gone now"
    );
}

#[test]
fn first_prompt_diff_line_empty() {
    assert_eq!(first_prompt_diff_line(""), "");
}
