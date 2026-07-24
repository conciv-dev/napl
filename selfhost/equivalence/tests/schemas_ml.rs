//! Equivalence gate for the `schemas::ml` module.
//!
//! This is the EXACT unit-test corpus of the hand-written `napl-core`
//! `schemas::ml` module (rust/crates/napl-core/src/schemas/ml.rs), replayed
//! against the NAPL-generated `schemas_ml` crate under
//! selfhost/.napl/src/rust/schemas_ml/. Each case asserts the same input ->
//! output the hand-written module asserts for itself.
//!
//! The generated crate surfaces its own error type where the hand-written module
//! uses `SchemaError`; equivalence is behavioral. Every `LineRange` here is the
//! NAPL-generated sibling `schemas_line_range` crate's type — the same generated
//! crate `schemas_ml` path-depends on — so this test also proves the
//! intra-workspace composition `schemas_ml -> schemas_line_range`.

use schemas_line_range::LineRange;
use schemas_ml::{ml_entries_at_body_line, parse_ml_entries, validate_ml, MlKind};
use serde_json::json;

#[test]
fn accepts_valid_document_with_normalized_range() {
    let ml = validate_ml(json!({
        "module": "todo-app",
        "target": "react",
        "entries": [
            { "promptLines": 18, "kind": "ambiguity", "message": "odd phrase", "reasoning": "unclear" }
        ]
    }))
    .unwrap();
    assert_eq!(ml.entries[0].prompt_lines, LineRange::new(18, 18));
    assert_eq!(ml.entries[0].kind, MlKind::Ambiguity);
    assert_eq!(ml.entries[0].suggestion, None);
}

#[test]
fn defaults_empty_entries() {
    let ml = validate_ml(json!({ "module": "m", "target": "react" })).unwrap();
    assert!(ml.entries.is_empty());
}

#[test]
fn accepts_all_kinds_including_no_op() {
    for kind in ["ambiguity", "assumption", "note", "no-op"] {
        let ml = validate_ml(json!({
            "module": "m", "target": "react",
            "entries": [{ "promptLines": [1, 1], "kind": kind, "message": "x" }]
        }))
        .unwrap();
        assert_eq!(ml.entries.len(), 1);
    }
}

#[test]
fn rejects_unknown_kind() {
    assert!(validate_ml(json!({
        "module": "m", "target": "react",
        "entries": [{ "promptLines": [1, 1], "kind": "bogus", "message": "x" }]
    }))
    .is_err());
}

#[test]
fn rejects_empty_message() {
    assert!(validate_ml(json!({
        "module": "m", "target": "react",
        "entries": [{ "promptLines": [1, 1], "kind": "note", "message": "" }]
    }))
    .is_err());
}

#[test]
fn parse_entries_list_and_non_list() {
    assert_eq!(
        parse_ml_entries(json!([
            { "promptLines": [2, 3], "kind": "assumption", "message": "a" }
        ]))
        .unwrap()
        .len(),
        1
    );
    assert!(parse_ml_entries(json!({})).unwrap().is_empty());
}

#[test]
fn parse_entries_throws_on_malformed() {
    assert!(parse_ml_entries(json!([
        { "promptLines": "nope", "kind": "note", "message": "x" }
    ]))
    .is_err());
}

#[test]
fn entries_at_body_line() {
    let ml = validate_ml(json!({
        "module": "m", "target": "react",
        "entries": [
            { "promptLines": [1, 2], "kind": "note", "message": "a" },
            { "promptLines": [5, 7], "kind": "ambiguity", "message": "b" }
        ]
    }))
    .unwrap();
    assert_eq!(
        ml_entries_at_body_line(&ml, 6)
            .iter()
            .map(|e| e.message.as_str())
            .collect::<Vec<_>>(),
        vec!["b"]
    );
    assert_eq!(
        ml_entries_at_body_line(&ml, 1)
            .iter()
            .map(|e| e.message.as_str())
            .collect::<Vec<_>>(),
        vec!["a"]
    );
    assert!(ml_entries_at_body_line(&ml, 4).is_empty());
}
