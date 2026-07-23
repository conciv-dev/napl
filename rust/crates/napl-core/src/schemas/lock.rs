//! Lock schema: model / backend / prompt-alias configuration.

use serde::{Deserialize, Serialize};

use crate::extensions::default_prompt_aliases;

use super::SchemaError;

/// The default model.
pub const DEFAULT_MODEL: &str = "claude-sonnet-5";

const ZWJ: char = '\u{200D}';

/// The code-generation backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Backend {
    ClaudeCli,
    AnthropicApi,
}

/// The default backend.
pub const DEFAULT_BACKEND: Backend = Backend::ClaudeCli;

fn default_backend() -> Backend {
    DEFAULT_BACKEND
}

/// The lock document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HlLock {
    pub model: String,
    #[serde(default = "default_backend")]
    pub backend: Backend,
    #[serde(
        rename = "promptAliases",
        skip_serializing_if = "Option::is_none",
        default
    )]
    pub prompt_aliases: Option<Vec<String>>,
}

/// Validate a single prompt alias, mirroring `promptAliasSchema`: it must start
/// with `.`, have 1-2 code points after the `.`, and contain no ZWJ.
fn validate_alias(value: &str) -> Result<(), SchemaError> {
    if !value.starts_with('.') {
        return Err(SchemaError::Validation(
            "a prompt alias must start with \".\"".to_string(),
        ));
    }
    let after_dot_codepoints = value.chars().skip(1).count();
    if !(1..=2).contains(&after_dot_codepoints) {
        return Err(SchemaError::Validation(
            "a prompt alias must have 1-2 code points after the \".\"".to_string(),
        ));
    }
    if value.contains(ZWJ) {
        return Err(SchemaError::Validation(
            "a prompt alias must not contain a ZWJ (zero-width joiner) sequence".to_string(),
        ));
    }
    Ok(())
}

/// Parse and validate a lock JSON string, mirroring `parseLock`.
pub fn parse_lock(raw: &str) -> Result<HlLock, SchemaError> {
    let lock: HlLock =
        serde_json::from_str(raw).map_err(|e| SchemaError::Deserialize(e.to_string()))?;
    if lock.model.is_empty() {
        return Err(SchemaError::Validation(
            "model must not be empty".to_string(),
        ));
    }
    if let Some(aliases) = &lock.prompt_aliases {
        for alias in aliases {
            validate_alias(alias)?;
        }
    }
    Ok(lock)
}

/// The effective prompt aliases: the lock's override or the curated default.
#[must_use]
pub fn resolve_prompt_aliases(lock: &HlLock) -> Vec<String> {
    lock.prompt_aliases
        .clone()
        .unwrap_or_else(default_prompt_aliases)
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn default_model_constant() {
        assert_eq!(DEFAULT_MODEL, "claude-sonnet-5");
    }

    #[test]
    fn prompt_aliases_default_to_curated_when_absent() {
        let lock = parse_lock(r#"{"model":"m"}"#).unwrap();
        assert!(lock.prompt_aliases.is_none());
        assert_eq!(resolve_prompt_aliases(&lock), default_prompt_aliases());
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
        // ".👨‍💻" as JSON escapes: man + ZWJ + laptop.
        assert!(parse_lock(r#"{"model":"m","promptAliases":[".👨‍💻"]}"#).is_err());
    }
}
