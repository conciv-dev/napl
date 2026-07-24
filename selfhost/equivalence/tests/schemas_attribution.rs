//! Equivalence gate for the `schemas::attribution` module.
//!
//! This is the EXACT unit-test corpus of the hand-written `napl-core`
//! `schemas::attribution` module (rust/crates/napl-core/src/schemas/attribution.rs),
//! replayed against the NAPL-generated `schemas_attribution` crate under
//! selfhost/.napl/src/rust/schemas_attribution/. Each case asserts the same
//! input -> output the hand-written module asserts for itself.
//!
//! The generated crate surfaces its own error type where the hand-written module
//! uses `SchemaError`; equivalence is behavioral (accept/reject and resolved
//! values). Every `LineRange` here is the NAPL-generated sibling
//! `schemas_line_range` crate's type — the same generated crate the
//! `schemas_attribution` crate path-depends on — so this test also proves the
//! intra-workspace composition `schemas_attribution -> schemas_line_range`.

use schemas_attribution::{
    entries_at_body_line, parse_attribution_entries, validate_attribution, Attribution,
};
use schemas_line_range::LineRange;
use serde_json::json;

fn sample() -> Attribution {
    validate_attribution(json!({
        "module": "greeting",
        "target": "typescript",
        "entries": [
            { "promptLines": [2, 2], "file": "greeting.ts", "lines": [1, 3], "note": "builds the greeting" },
            { "promptLines": [3, 4], "file": "greeting.ts", "lines": [5, 7], "note": "trims whitespace" },
            { "promptLines": [3, 3], "file": "greeting.test.ts", "lines": [10, 12], "note": "covers trimming" }
        ]
    }))
    .unwrap()
}

#[test]
fn validates_well_formed_document() {
    assert_eq!(sample().entries.len(), 3);
}

#[test]
fn rejects_missing_file() {
    let err = validate_attribution(json!({
        "module": "m", "target": "t",
        "entries": [{ "promptLines": [1, 1], "lines": [1, 1], "note": "x" }]
    }));
    assert!(err.is_err());
}

#[test]
fn rejects_empty_file() {
    let err = validate_attribution(json!({
        "module": "m", "target": "t",
        "entries": [{ "promptLines": [1, 1], "file": "", "lines": [1, 1] }]
    }));
    assert!(err.is_err());
}

#[test]
fn rejects_non_integer_line_range() {
    let err = validate_attribution(json!({
        "module": "m", "target": "t",
        "entries": [{ "promptLines": [1, 1], "file": "a.ts", "lines": [1.5, 2], "note": "x" }]
    }));
    assert!(err.is_err());
}

#[test]
fn normalizes_single_line_range() {
    let entries = parse_attribution_entries(json!([
        { "promptLines": [8], "file": "a.ts", "lines": 3, "note": "single line" }
    ]))
    .unwrap();
    assert_eq!(entries[0].prompt_lines, LineRange::new(8, 8));
    assert_eq!(entries[0].lines, LineRange::new(3, 3));
}

#[test]
fn default_note_is_empty() {
    let entries = parse_attribution_entries(json!([
        { "promptLines": [1, 1], "file": "a.ts", "lines": [1, 1] }
    ]))
    .unwrap();
    assert_eq!(entries[0].note, "");
}

#[test]
fn parse_entries_non_list_is_empty() {
    assert!(parse_attribution_entries(json!({})).unwrap().is_empty());
}

#[test]
fn throws_on_malformed_list() {
    assert!(parse_attribution_entries(json!([
        { "promptLines": "nope", "file": "a", "lines": [1, 2] }
    ]))
    .is_err());
}

#[test]
fn entries_at_body_line_range_lookup() {
    let attribution = sample();
    let mut at_three: Vec<&str> = entries_at_body_line(&attribution, 3)
        .iter()
        .map(|e| e.note.as_str())
        .collect();
    at_three.sort_unstable();
    assert_eq!(at_three, vec!["covers trimming", "trims whitespace"]);

    assert_eq!(
        entries_at_body_line(&attribution, 2)
            .iter()
            .map(|e| e.note.as_str())
            .collect::<Vec<_>>(),
        vec!["builds the greeting"]
    );
    assert!(entries_at_body_line(&attribution, 9).is_empty());
}
