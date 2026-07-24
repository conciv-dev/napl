//! Equivalence gate for the `reverse` module.
//!
//! This is the EXACT unit-test corpus of the hand-written `napl-core` `reverse`
//! module (rust/crates/napl-core/src/reverse.rs), replayed against the
//! NAPL-generated `reverse` crate under selfhost/.napl/src/rust/reverse/. Each
//! case asserts the same input -> output the hand-written module asserts for
//! itself.
//!
//! The `AttributionEntry` and `LineRange` used here are the NAPL-generated
//! sibling `schemas_attribution` and `schemas_line_range` crates' types — the
//! same generated crates `reverse` path-depends on (alongside `body_lines`) — so
//! this test also proves the intra-workspace composition
//! `reverse -> {body_lines, schemas_attribution, schemas_line_range}`.

use reverse::{
    code_lens_title, dedupe_matches, is_file_drifted, parse_generated_path, prompt_absolute_lines,
    reverse_matches, AttributionSource, GeneratedPathInfo,
};
use schemas_attribution::AttributionEntry;
use schemas_line_range::LineRange;

fn entry(pl: (u32, u32), file: &str, cl: (u32, u32), note: &str) -> AttributionEntry {
    AttributionEntry {
        prompt_lines: LineRange::new(pl.0, pl.1),
        file: file.to_string(),
        lines: LineRange::new(cl.0, cl.1),
        note: note.to_string(),
    }
}

fn sources() -> Vec<AttributionSource> {
    vec![AttributionSource {
        module: "greeting".to_string(),
        target: "typescript".to_string(),
        prompt_files: vec!["examples/greeting.napl".to_string()],
        entries: vec![
            entry((3, 4), "src/greeting.ts", (1, 1), "greet function signature"),
            entry((7, 7), "src/greeting.ts", (2, 2), "trims name whitespace"),
            entry((8, 8), "src/greeting.ts", (3, 5), "rejects empty name"),
            entry((6, 6), "src/greeting.ts", (6, 6), "builds greeting message"),
            entry((7, 7), "src/greeting.test.ts", (9, 11), "test trimming"),
        ],
    }]
}

#[test]
fn splits_generated_path() {
    assert_eq!(
        parse_generated_path(".napl/src/typescript/src/greeting.ts"),
        Some(GeneratedPathInfo {
            target: "typescript".to_string(),
            target_rel_path: "src/greeting.ts".to_string(),
        })
    );
}

#[test]
fn normalizes_windows_separators() {
    assert_eq!(
        parse_generated_path(".napl\\src\\typescript\\src\\greeting.ts"),
        Some(GeneratedPathInfo {
            target: "typescript".to_string(),
            target_rel_path: "src/greeting.ts".to_string(),
        })
    );
}

#[test]
fn returns_none_for_non_generated_paths() {
    assert_eq!(parse_generated_path("examples/greeting.napl"), None);
    assert_eq!(parse_generated_path(".napl/src/typescript"), None);
    assert_eq!(parse_generated_path("src/greeting.ts"), None);
}

#[test]
fn finds_entry_whose_range_contains_line() {
    let matches = reverse_matches(&sources(), "typescript", "src/greeting.ts", Some(2));
    assert_eq!(
        matches.iter().map(|m| m.note.as_str()).collect::<Vec<_>>(),
        vec!["trims name whitespace"]
    );
    assert_eq!(matches[0].prompt_lines, LineRange::new(7, 7));
    assert_eq!(matches[0].prompt_file, "examples/greeting.napl");
}

#[test]
fn matches_multiline_code_range() {
    let matches = reverse_matches(&sources(), "typescript", "src/greeting.ts", Some(4));
    assert_eq!(
        matches.iter().map(|m| m.note.as_str()).collect::<Vec<_>>(),
        vec!["rejects empty name"]
    );
}

#[test]
fn returns_nothing_when_target_or_file_differs() {
    assert!(reverse_matches(&sources(), "swift", "src/greeting.ts", Some(2)).is_empty());
    assert!(reverse_matches(&sources(), "typescript", "src/other.ts", Some(2)).is_empty());
}

#[test]
fn returns_all_entries_when_code_line_null() {
    let matches = reverse_matches(&sources(), "typescript", "src/greeting.ts", None);
    assert_eq!(matches.len(), 4);
}

#[test]
fn merges_multiple_prompts_into_separate_matches() {
    let multi = vec![AttributionSource {
        module: "greeting".to_string(),
        target: "typescript".to_string(),
        prompt_files: vec!["examples/a.napl".to_string(), "examples/b.napl".to_string()],
        entries: vec![entry((1, 1), "src/x.ts", (4, 4), "shared")],
    }];
    let matches = reverse_matches(&multi, "typescript", "src/x.ts", Some(4));
    assert_eq!(
        matches
            .iter()
            .map(|m| m.prompt_file.as_str())
            .collect::<Vec<_>>(),
        vec!["examples/a.napl", "examples/b.napl"]
    );
}

#[test]
fn converts_body_relative_prompt_lines() {
    assert_eq!(prompt_absolute_lines(12, LineRange::new(7, 7)), (18, 18));
    assert_eq!(prompt_absolute_lines(12, LineRange::new(3, 4)), (14, 15));
    assert_eq!(prompt_absolute_lines(3, LineRange::new(1, 1)), (3, 3));
    assert_eq!(prompt_absolute_lines(3, LineRange::new(2, 2)), (4, 4));
}

#[test]
fn drift_detection() {
    assert!(is_file_drifted(Some("aaa"), "bbb"));
    assert!(!is_file_drifted(Some("aaa"), "aaa"));
    assert!(!is_file_drifted(None, "bbb"));
}

#[test]
fn lens_title_formatting() {
    assert_eq!(
        code_lens_title("greeting.napl", 19, "trims name whitespace"),
        "⇠ greeting.napl:19 — trims name whitespace"
    );
    assert_eq!(
        code_lens_title("greeting.napl", 19, ""),
        "⇠ greeting.napl:19"
    );
}

#[test]
fn dedupe_collapses_duplicate_prompt_spans() {
    let matches = reverse_matches(&sources(), "typescript", "src/greeting.ts", None);
    let mut with_dupes = matches.clone();
    with_dupes.extend(matches.clone());
    assert_eq!(dedupe_matches(&with_dupes).len(), matches.len());
}
