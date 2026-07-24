//! Stage1 adapter over the NAPL-generated `yaml` crate.

pub use gen_yaml::{attribution_to_yaml, ir_to_yaml, ml_to_yaml, to_yaml_document, Yaml};

#[cfg(test)]
mod tests {
    use super::*;

    fn scalar(s: &str) -> String {
        let doc = to_yaml_document(&Yaml::Map(vec![(
            "v".to_string(),
            Yaml::Str(s.to_string()),
        )]));
        doc.strip_prefix("v: ").unwrap().trim_end().to_string()
    }

    #[test]
    fn plain_when_possible() {
        assert_eq!(
            scalar("Returns the string Hello, name!"),
            "Returns the string Hello, name!"
        );
        assert_eq!(scalar("nothing to add"), "nothing to add");
        assert_eq!(scalar("it's fine"), "it's fine");
        assert_eq!(
            scalar("state the exact emphasis, e.g. all caps"),
            "state the exact emphasis, e.g. all caps"
        );
        assert_eq!(scalar("yes"), "yes");
        assert_eq!(scalar("colon:nospace"), "colon:nospace");
    }

    #[test]
    fn double_quoted_cases() {
        assert_eq!(
            scalar("greet(name: string): string"),
            "\"greet(name: string): string\""
        );
        assert_eq!(scalar("#leadinghash"), "\"#leadinghash\"");
        assert_eq!(scalar(" leadingspace"), "\" leadingspace\"");
        assert_eq!(scalar("trailingspace "), "\"trailingspace \"");
        assert_eq!(scalar("true"), "\"true\"");
        assert_eq!(scalar("123"), "\"123\"");
        assert_eq!(scalar("null"), "\"null\"");
        assert_eq!(scalar("a: b"), "\"a: b\"");
        assert_eq!(scalar("has # hash"), "\"has # hash\"");
        assert_eq!(scalar("- dashlead"), "\"- dashlead\"");
        assert_eq!(scalar("end colon:"), "\"end colon:\"");
        assert_eq!(scalar("1.5"), "\"1.5\"");
        assert_eq!(scalar("0x10"), "\"0x10\"");
        assert_eq!(scalar(".inf"), "\".inf\"");
        assert_eq!(scalar("~"), "\"~\"");
        assert_eq!(
            scalar("only ' apostrophe: with colon"),
            "\"only ' apostrophe: with colon\""
        );
    }

    #[test]
    fn single_quoted_case() {
        assert_eq!(scalar("\"loudly\" is vague"), "'\"loudly\" is vague'");
    }

    #[test]
    fn empty_string_double_quoted() {
        assert_eq!(scalar(""), "\"\"");
    }

    #[test]
    fn one_one_five_is_not_special_when_yes_no() {
        assert_eq!(scalar("on"), "on");
        assert_eq!(scalar("Off"), "Off");
        assert_eq!(scalar("no"), "no");
    }
}

#[cfg(test)]
mod doc_tests {
    use super::*;
    use crate::schemas::{AttributionEntry, Ir, IrFunction, LineRange, Ml, MlEntry, MlKind};

    #[test]
    fn attribution_matches_pinned_bytes() {
        let attribution = crate::schemas::Attribution {
            module: "greeting".to_string(),
            target: "typescript".to_string(),
            entries: vec![AttributionEntry {
                prompt_lines: LineRange::new(1, 1),
                file: "greet.ts".to_string(),
                lines: LineRange::new(1, 1),
                note: "builds the greeting string".to_string(),
            }],
        };
        assert_eq!(
            attribution_to_yaml(&attribution),
            "module: greeting\ntarget: typescript\nentries:\n  - promptLines:\n      - 1\n      - 1\n    file: greet.ts\n    lines:\n      - 1\n      - 1\n    note: builds the greeting string\n"
        );
    }

    #[test]
    fn ir_matches_pinned_bytes() {
        let ir = Ir {
            module: "greeting".to_string(),
            deps: Vec::new(),
            types: Vec::new(),
            functions: vec![IrFunction {
                name: "greet".to_string(),
                signature: "greet(name: string): string".to_string(),
                behavior: "Returns the string Hello, name!".to_string(),
            }],
            tests: Vec::new(),
        };
        assert_eq!(
            ir_to_yaml(&ir),
            "module: greeting\ndeps: []\ntypes: []\nfunctions:\n  - name: greet\n    signature: \"greet(name: string): string\"\n    behavior: Returns the string Hello, name!\ntests: []\n"
        );
    }

    #[test]
    fn ml_empty_matches_pinned_bytes() {
        let ml = Ml {
            module: "greeting".to_string(),
            target: "typescript".to_string(),
            entries: Vec::new(),
        };
        assert_eq!(
            ml_to_yaml(&ml),
            "module: greeting\ntarget: typescript\nentries: []\n"
        );
    }

    #[test]
    fn ml_kinds_match_pinned_bytes() {
        let ml = Ml {
            module: "greeting".to_string(),
            target: "typescript".to_string(),
            entries: vec![
                MlEntry {
                    prompt_lines: LineRange::new(1, 1),
                    kind: MlKind::Ambiguity,
                    message: "\"loudly\" is vague".to_string(),
                    reasoning: "unclear whether to uppercase, add punctuation, or both".to_string(),
                    suggestion: Some("state the exact emphasis, e.g. all caps".to_string()),
                },
                MlEntry {
                    prompt_lines: LineRange::new(1, 1),
                    kind: MlKind::NoOp,
                    message: "requirement already satisfied by existing code".to_string(),
                    reasoning: "the greeting is already emphatic enough; no edit was needed"
                        .to_string(),
                    suggestion: None,
                },
            ],
        };
        let expected = "module: greeting\ntarget: typescript\nentries:\n  - promptLines:\n      - 1\n      - 1\n    kind: ambiguity\n    message: '\"loudly\" is vague'\n    reasoning: unclear whether to uppercase, add punctuation, or both\n    suggestion: state the exact emphasis, e.g. all caps\n  - promptLines:\n      - 1\n      - 1\n    kind: no-op\n    message: requirement already satisfied by existing code\n    reasoning: the greeting is already emphatic enough; no edit was needed\n";
        assert_eq!(ml_to_yaml(&ml), expected);
    }
}
