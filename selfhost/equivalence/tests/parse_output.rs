//! Equivalence gate for the `parse_output` module.
//!
//! This is the EXACT unit-test corpus of the hand-written `napl-core`
//! `parse_output` module (rust/crates/napl-core/src/parse_output.rs), replayed
//! against the NAPL-generated `parse_output` crate under
//! selfhost/.napl/src/rust/parse_output/. Each case asserts the same input ->
//! output the hand-written module asserts for itself.

use parse_output::extract_yaml;

#[test]
fn extracts_yaml_fence() {
    let text = "```yaml\nmodule: greeting\ntests: []\n```\n";
    assert_eq!(extract_yaml(text), "module: greeting\ntests: []");
}

#[test]
fn extracts_bare_fence() {
    assert_eq!(extract_yaml("```\n[]\n```"), "[]");
}

#[test]
fn falls_back_to_trimmed_text() {
    assert_eq!(extract_yaml("  module: x  "), "module: x");
}
