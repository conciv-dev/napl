//! Equivalence gate for the `schemas::frontmatter` module.
//!
//! This is the EXACT unit-test corpus of the hand-written `napl-core`
//! `schemas::frontmatter` module (rust/crates/napl-core/src/schemas/frontmatter.rs),
//! replayed against the NAPL-generated `schemas_frontmatter` crate under
//! selfhost/.napl/src/rust/schemas_frontmatter/. Each case asserts the same
//! input -> output the hand-written module asserts for itself.

use schemas_frontmatter::parse_frontmatter;

const VALID: &str = "---\nmodule: auth/session\ndeps: [auth/tokens]\ntargets: [typescript]\ntests:\n  - name: expired token rejected\n    given: { token: expired }\n    expect: { error: SESSION_EXPIRED }\n---\nManage user sessions. Sessions expire after 30 minutes.\n";

#[test]
fn parses_frontmatter_and_body() {
    let parsed = parse_frontmatter(VALID).unwrap();
    assert_eq!(parsed.frontmatter.module, "auth/session");
    assert_eq!(parsed.frontmatter.deps, vec!["auth/tokens"]);
    assert_eq!(parsed.frontmatter.targets, vec!["typescript"]);
    assert_eq!(parsed.frontmatter.tests.len(), 1);
    assert_eq!(parsed.frontmatter.tests[0].name, "expired token rejected");
    assert_eq!(
        parsed.frontmatter.tests[0].given["token"],
        serde_yaml::Value::from("expired")
    );
    assert_eq!(
        parsed.frontmatter.tests[0].expect["error"],
        serde_yaml::Value::from("SESSION_EXPIRED")
    );
    assert!(parsed.body.starts_with("Manage user sessions."));
}

#[test]
fn applies_defaults_for_optional_fields() {
    let parsed = parse_frontmatter("---\nmodule: solo\n---\nBody here.\n").unwrap();
    assert!(parsed.frontmatter.deps.is_empty());
    assert!(parsed.frontmatter.targets.is_empty());
    assert!(parsed.frontmatter.tests.is_empty());
    assert_eq!(parsed.frontmatter.crate_name, None);
    assert_eq!(parsed.body, "Body here.\n");
}

#[test]
fn captures_optional_crate_key_into_crate_name() {
    let parsed = parse_frontmatter("---\nmodule: alpha\ncrate: shared\n---\nBody.\n").unwrap();
    assert_eq!(parsed.frontmatter.module, "alpha");
    assert_eq!(parsed.frontmatter.crate_name, Some("shared".to_string()));
}

#[test]
fn throws_when_frontmatter_missing() {
    assert!(parse_frontmatter("no frontmatter here").is_err());
}

#[test]
fn throws_when_module_absent() {
    assert!(parse_frontmatter("---\ndeps: []\n---\nbody").is_err());
}

#[test]
fn throws_when_module_empty() {
    assert!(parse_frontmatter("---\nmodule: \"\"\n---\nbody").is_err());
}

#[test]
fn strips_a_single_leading_blank_line() {
    let parsed = parse_frontmatter("---\nmodule: m\n---\n\nActual body.\n").unwrap();
    assert_eq!(parsed.body, "Actual body.\n");
}
