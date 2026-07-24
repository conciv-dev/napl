//! Stage1 adapter over the NAPL-generated `schemas_frontmatter` crate. The
//! generated `FrontmatterError` is mapped, variant by variant, to the exact
//! `SchemaError` message text the CLI's observable contract depends on (the
//! generated crate's own Display strings reword them, which the conformance
//! corpus pins — this mapping is the error-enum seam bridge).

use super::SchemaError;

pub use schemas_frontmatter::{Frontmatter, ParsedPrompt, PromptTest};

pub fn parse_frontmatter(raw: &str) -> Result<ParsedPrompt, SchemaError> {
    schemas_frontmatter::parse_frontmatter(raw).map_err(frontmatter_error_to_schema)
}

fn frontmatter_error_to_schema(error: schemas_frontmatter::FrontmatterError) -> SchemaError {
    use schemas_frontmatter::FrontmatterError as E;
    match error {
        E::MissingFrontmatter => SchemaError::Validation(
            "missing YAML frontmatter: a prompt file must start with a --- delimited block"
                .to_string(),
        ),
        E::InvalidYaml(inner) => {
            SchemaError::Deserialize(format!("invalid YAML frontmatter: {inner}"))
        }
        E::EmptyModule => SchemaError::Validation("module must not be empty".to_string()),
        E::InvalidTestField { field, .. } => {
            SchemaError::Validation(format!("{field} must be a mapping"))
        }
    }
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
