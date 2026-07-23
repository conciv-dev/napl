//! Prompt frontmatter: strict `---` delimiter parsing plus YAML schema.

use serde::Deserialize;

use super::SchemaError;

fn default_record() -> serde_yaml::Value {
    serde_yaml::Value::Mapping(serde_yaml::Mapping::new())
}

/// A single prompt test case in the frontmatter.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct PromptTest {
    pub name: String,
    #[serde(default = "default_record")]
    pub given: serde_yaml::Value,
    #[serde(default = "default_record")]
    pub expect: serde_yaml::Value,
}

/// The parsed frontmatter block.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Frontmatter {
    pub module: String,
    #[serde(default)]
    pub deps: Vec<String>,
    #[serde(default)]
    pub targets: Vec<String>,
    #[serde(default)]
    pub tests: Vec<PromptTest>,
}

/// A prompt file split into its validated frontmatter and body.
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedPrompt {
    pub frontmatter: Frontmatter,
    pub body: String,
}

/// Match the strict opening `^---\r?\n` and the closing `\r?\n---\r?\n?`,
/// returning `(yaml_text, body)`. Mirrors the TS `FRONTMATTER_RE`.
fn split_delimited(raw: &str) -> Option<(&str, &str)> {
    let after_open = raw
        .strip_prefix("---\r\n")
        .or_else(|| raw.strip_prefix("---\n"))?;
    let bytes = after_open.as_bytes();
    let mut i = 0;
    while i < after_open.len() {
        if bytes[i] == b'\n' && after_open[i + 1..].starts_with("---") {
            let sep_start = if i > 0 && bytes[i - 1] == b'\r' {
                i - 1
            } else {
                i
            };
            let mut j = i + 1 + 3;
            if j < after_open.len() && bytes[j] == b'\r' {
                j += 1;
            }
            if j < after_open.len() && bytes[j] == b'\n' {
                j += 1;
            }
            return Some((&after_open[..sep_start], &after_open[j..]));
        }
        i += 1;
    }
    None
}

/// Remove a single leading blank line, mirroring `body.replace(/^\s*\n/, '')`.
fn strip_leading_blank(body: &str) -> String {
    let ws_end: usize = body
        .char_indices()
        .take_while(|(_, c)| c.is_whitespace())
        .map(|(i, c)| i + c.len_utf8())
        .last()
        .unwrap_or(0);
    let leading = &body[..ws_end];
    match leading.rfind('\n') {
        Some(nl) => body[nl + 1..].to_string(),
        None => body.to_string(),
    }
}

fn validate_record(value: &serde_yaml::Value, field: &str) -> Result<(), SchemaError> {
    if value.is_mapping() {
        Ok(())
    } else {
        Err(SchemaError::Validation(format!(
            "{field} must be a mapping"
        )))
    }
}

/// Parse a prompt file into frontmatter + body, mirroring `parseFrontmatter`.
pub fn parse_frontmatter(raw: &str) -> Result<ParsedPrompt, SchemaError> {
    let Some((yaml_text, body)) = split_delimited(raw) else {
        return Err(SchemaError::Validation(
            "missing YAML frontmatter: a prompt file must start with a --- delimited block"
                .to_string(),
        ));
    };
    let value: serde_yaml::Value = serde_yaml::from_str(yaml_text)
        .map_err(|e| SchemaError::Deserialize(format!("invalid YAML frontmatter: {e}")))?;
    let value = if value.is_null() {
        default_record()
    } else {
        value
    };
    let frontmatter: Frontmatter = serde_yaml::from_value(value)
        .map_err(|e| SchemaError::Deserialize(format!("invalid frontmatter: {e}")))?;
    if frontmatter.module.is_empty() {
        return Err(SchemaError::Validation(
            "module must not be empty".to_string(),
        ));
    }
    for test in &frontmatter.tests {
        validate_record(&test.given, "given")?;
        validate_record(&test.expect, "expect")?;
    }
    Ok(ParsedPrompt {
        frontmatter,
        body: strip_leading_blank(body),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert_eq!(parsed.body, "Body here.\n");
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
}
