//! Equivalence gate for the `schemas::lock` module.
//!
//! This is the EXACT unit-test corpus of the hand-written `napl-core`
//! `schemas::lock` module (rust/crates/napl-core/src/schemas/lock.rs), replayed
//! against the NAPL-generated `schemas_lock` crate under
//! selfhost/.napl/src/rust/schemas_lock/. Each case asserts the same input ->
//! output the hand-written module asserts for itself.
//!
//! The generated crate surfaces its own `LockError` type where the hand-written
//! module uses `SchemaError`; equivalence is behavioral (accept/reject and the
//! resolved values), not type-identical. The generated `resolve_prompt_aliases`
//! curated default comes from the NAPL-generated sibling `extensions` crate —
//! the same generated crate this harness gates directly — so this test also
//! proves the intra-workspace composition `schemas_lock -> extensions`.

use schemas_lock::{
    default_agent_config, parse_lock, resolve_agent_config, resolve_prompt_aliases, AgentPreset,
    Backend, DEFAULT_BACKEND, DEFAULT_MODEL,
};

#[test]
fn defaults_backend_to_claude_cli() {
    let lock = parse_lock(r#"{"model":"claude-sonnet-5"}"#).unwrap();
    assert_eq!(lock.backend, Backend::ClaudeCli);
    assert_eq!(DEFAULT_BACKEND, Backend::ClaudeCli);
}

#[test]
fn keeps_explicit_anthropic_api() {
    let lock = parse_lock(r#"{"model":"claude-sonnet-5","backend":"anthropic-api"}"#).unwrap();
    assert_eq!(lock.backend, Backend::AnthropicApi);
}

#[test]
fn keeps_explicit_claude_cli() {
    let lock = parse_lock(r#"{"model":"claude-opus-5","backend":"claude-cli"}"#).unwrap();
    assert_eq!(lock.backend, Backend::ClaudeCli);
    assert_eq!(lock.model, "claude-opus-5");
}

#[test]
fn rejects_unknown_backend() {
    assert!(parse_lock(r#"{"model":"x","backend":"openai"}"#).is_err());
}

#[test]
fn rejects_corrupt_json() {
    assert!(parse_lock("{not json").is_err());
}

#[test]
fn rejects_empty_model() {
    assert!(parse_lock(r#"{"model":""}"#).is_err());
}

#[test]
fn default_model_constant() {
    assert_eq!(DEFAULT_MODEL, "claude-sonnet-5");
}

#[test]
fn prompt_aliases_default_to_curated_when_absent() {
    let lock = parse_lock(r#"{"model":"m"}"#).unwrap();
    assert!(lock.prompt_aliases.is_none());
    assert_eq!(
        resolve_prompt_aliases(&lock),
        extensions::default_prompt_aliases()
    );
}

#[test]
fn accepts_valid_override_verbatim() {
    let lock = parse_lock(r#"{"model":"m","promptAliases":[".🧑",".🤠"]}"#).unwrap();
    assert_eq!(
        lock.prompt_aliases,
        Some(vec![".\u{1F9D1}".to_string(), ".\u{1F920}".to_string()])
    );
    assert_eq!(
        resolve_prompt_aliases(&lock),
        vec![".\u{1F9D1}".to_string(), ".\u{1F920}".to_string()]
    );
}

#[test]
fn rejects_alias_without_dot() {
    assert!(parse_lock(r#"{"model":"m","promptAliases":["🧑"]}"#).is_err());
}

#[test]
fn rejects_alias_with_more_than_two_codepoints() {
    assert!(parse_lock(r#"{"model":"m","promptAliases":[".abc"]}"#).is_err());
}

#[test]
fn rejects_zwj_sequence() {
    assert!(parse_lock(r#"{"model":"m","promptAliases":[".👨‍💻"]}"#).is_err());
}

#[test]
fn agent_defaults_to_claude_when_absent() {
    let lock = parse_lock(r#"{"model":"m"}"#).unwrap();
    assert!(lock.agent.is_none());
    assert_eq!(resolve_agent_config(&lock), default_agent_config());
    assert_eq!(resolve_agent_config(&lock).preset, AgentPreset::Claude);
}

#[test]
fn accepts_codex_preset() {
    let lock = parse_lock(r#"{"model":"m","agent":{"preset":"codex"}}"#).unwrap();
    assert_eq!(lock.agent.unwrap().preset, AgentPreset::Codex);
}

#[test]
fn accepts_custom_preset_with_command() {
    let lock = parse_lock(
        r#"{"model":"m","agent":{"preset":"custom","command":["mycli","--task-file","{task}"]}}"#,
    )
    .unwrap();
    let agent = lock.agent.unwrap();
    assert_eq!(agent.preset, AgentPreset::Custom);
    assert_eq!(
        agent.command,
        Some(vec![
            "mycli".to_string(),
            "--task-file".to_string(),
            "{task}".to_string()
        ])
    );
}

#[test]
fn rejects_unknown_preset() {
    assert!(parse_lock(r#"{"model":"m","agent":{"preset":"gpt"}}"#).is_err());
}

#[test]
fn rejects_custom_without_command() {
    assert!(parse_lock(r#"{"model":"m","agent":{"preset":"custom"}}"#).is_err());
}

#[test]
fn rejects_custom_with_empty_command() {
    assert!(parse_lock(r#"{"model":"m","agent":{"preset":"custom","command":[]}}"#).is_err());
}

#[test]
fn rejects_claude_preset_with_command() {
    assert!(parse_lock(r#"{"model":"m","agent":{"preset":"claude","command":["x"]}}"#).is_err());
}

#[test]
fn rejects_unknown_agent_field() {
    assert!(parse_lock(r#"{"model":"m","agent":{"preset":"claude","extra":1}}"#).is_err());
}
