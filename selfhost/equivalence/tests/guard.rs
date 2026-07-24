//! Equivalence gate for the `guard` module.
//!
//! This is the EXACT unit-test corpus of the hand-written `napl-core` `guard`
//! module (rust/crates/napl-core/src/guard.rs), replayed against the
//! NAPL-generated `guard` crate under selfhost/.napl/src/rust/guard/. Each case
//! asserts the same input -> output the hand-written module asserts for itself,
//! including the byte-exact settings snippet.

use guard::{claude_settings_snippet, merge_claude_settings, SettingsMergeAction};

#[test]
fn snippet_is_pretty_with_trailing_newline() {
    assert_eq!(
        claude_settings_snippet(),
        "{\n  \"permissions\": {\n    \"deny\": [\n      \"Edit(/.napl/src/**)\"\n    ]\n  }\n}\n"
    );
}

#[test]
fn create_when_absent() {
    let result = merge_claude_settings(None);
    assert_eq!(result.action, SettingsMergeAction::Create);
    assert_eq!(result.content, Some(claude_settings_snippet()));
}

#[test]
fn unchanged_when_rule_present() {
    let existing = claude_settings_snippet();
    let result = merge_claude_settings(Some(&existing));
    assert_eq!(result.action, SettingsMergeAction::Unchanged);
    assert!(result.content.is_none());
}

#[test]
fn update_merges_without_clobbering() {
    let existing = "{\n  \"other\": true,\n  \"permissions\": {\n    \"allow\": [\"Read\"]\n  }\n}";
    let result = merge_claude_settings(Some(existing));
    assert_eq!(result.action, SettingsMergeAction::Update);
    let content = result.content.unwrap();
    assert!(content.contains("\"other\": true"));
    assert!(content.contains("\"allow\""));
    assert!(content.contains("Edit(/.napl/src/**)"));
    assert!(content.ends_with("}\n"));
}

#[test]
fn manual_on_garbage() {
    assert_eq!(
        merge_claude_settings(Some("not json")).action,
        SettingsMergeAction::Manual
    );
    assert_eq!(
        merge_claude_settings(Some("[1,2,3]")).action,
        SettingsMergeAction::Manual
    );
    assert_eq!(
        merge_claude_settings(Some("{\"permissions\": 5}")).action,
        SettingsMergeAction::Manual
    );
}
