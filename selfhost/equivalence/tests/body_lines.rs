//! Equivalence gate for the self-host pilot.
//!
//! This file is the EXACT unit-test corpus of the hand-written `napl-core`
//! `body_lines` module (rust/crates/napl-core/src/body_lines.rs), replayed
//! against the NAPL-generated `body_lines` crate under
//! selfhost/.napl/src/rust/. Every case asserts the same input -> output pair
//! the hand-written module asserts for itself. The generated function
//! `body_line_for_doc_line` returns `Option<u64>` where the hand-written one
//! returns `Option<usize>`; the values compared are identical, which is the
//! point — equivalence is behavioral, not byte- or type-identical.

use body_lines::{body_line_for_doc_line, number_lines, prompt_body_lines};

fn raw() -> &'static str {
    "---\nmodule: greeting\n---\nFirst body line.\nSecond body line."
}

#[test]
fn locates_body_start_after_frontmatter() {
    let body = prompt_body_lines(raw());
    assert_eq!(body.body_start_line, 3);
    assert_eq!(body.lines[0], "First body line.");
}

#[test]
fn maps_doc_lines_to_body_lines() {
    let body = prompt_body_lines(raw());
    assert_eq!(body_line_for_doc_line(&body, 3), Some(1));
    assert_eq!(body_line_for_doc_line(&body, 4), Some(2));
    assert_eq!(body_line_for_doc_line(&body, 2), None);
    assert_eq!(body_line_for_doc_line(&body, 99), None);
}

#[test]
fn numbers_lines_1_based() {
    assert_eq!(
        number_lines(&["a".to_string(), "b".to_string()]),
        "1: a\n2: b"
    );
}

#[test]
fn no_frontmatter_treats_all_as_body() {
    let body = prompt_body_lines("just a body\nsecond line");
    assert_eq!(body.body_start_line, 0);
    assert_eq!(body.lines.len(), 2);
    assert_eq!(body_line_for_doc_line(&body, 0), Some(1));
    assert_eq!(body_line_for_doc_line(&body, 1), Some(2));
}

#[test]
fn empty_body_after_frontmatter() {
    let body = prompt_body_lines("---\nmodule: m\n---\n");
    assert_eq!(body.body_start_line, 3);
    assert_eq!(body.lines, vec![String::new()]);
}
