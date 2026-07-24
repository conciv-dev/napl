//! Equivalence gate for the `schemas::line_range` module.
//!
//! This is the EXACT unit-test corpus of the hand-written `napl-core`
//! `schemas::line_range` module (rust/crates/napl-core/src/schemas/line_range.rs),
//! replayed against the NAPL-generated `schemas_line_range` crate under
//! selfhost/.napl/src/rust/schemas_line_range/. Each case asserts the same
//! input -> output the hand-written module asserts for itself.

use schemas_line_range::LineRange;

fn parse(v: serde_json::Value) -> Result<LineRange, serde_json::Error> {
    serde_json::from_value(v)
}

#[test]
fn scalar_normalizes_to_pair() {
    assert_eq!(parse(serde_json::json!(8)).unwrap(), LineRange::new(8, 8));
}

#[test]
fn single_element_array_normalizes() {
    assert_eq!(parse(serde_json::json!([8])).unwrap(), LineRange::new(8, 8));
}

#[test]
fn two_element_array_passes_through() {
    assert_eq!(parse(serde_json::json!([5, 7])).unwrap(), LineRange::new(5, 7));
}

#[test]
fn rejects_zero() {
    assert!(parse(serde_json::json!(0)).is_err());
    assert!(parse(serde_json::json!([0, 2])).is_err());
}

#[test]
fn rejects_non_integer() {
    assert!(parse(serde_json::json!(1.5)).is_err());
    assert!(parse(serde_json::json!([1.5, 2])).is_err());
}

#[test]
fn accepts_integral_float() {
    assert_eq!(parse(serde_json::json!(2.0)).unwrap(), LineRange::new(2, 2));
}

#[test]
fn rejects_three_element_array() {
    assert!(parse(serde_json::json!([1, 2, 3])).is_err());
}

#[test]
fn rejects_string() {
    assert!(parse(serde_json::json!("nope")).is_err());
}
