//! A focused YAML emitter that reproduces the output of the JavaScript
//! `yaml` package (`eemeli/yaml`) `stringify` for the document shapes NAPL
//! writes: attribution, IR, and machine-layer files.
//!
//! The corpus pins these files byte-for-byte, so this emitter matches
//! `eemeli/yaml`'s block style (2-space indent, `[]`/`{}` for empty
//! collections) and its scalar-style heuristics (plain when possible; single
//! quotes only for a plain-rejected string that contains a double quote and no
//! single quote; double quotes otherwise; core-schema resolvable scalars such
//! as `true`/`123`/`null` are quoted).

use crate::schemas::{Attribution, Ir, Ml, MlKind};

/// A minimal YAML value model used only for serialization.
#[derive(Debug, Clone, PartialEq)]
pub enum Yaml {
    /// A null scalar.
    Null,
    /// A boolean scalar.
    Bool(bool),
    /// An integer scalar.
    Int(i64),
    /// A floating-point scalar.
    Float(f64),
    /// A string scalar.
    Str(String),
    /// A block sequence.
    Seq(Vec<Yaml>),
    /// A block mapping, key order preserved.
    Map(Vec<(String, Yaml)>),
}

const FORBIDDEN_FIRST: &str = ",[]{}#&*!|>'\"%@`";

fn is_control(c: char) -> bool {
    (c as u32) < 0x20 && c != '\n' || c == '\u{7f}'
}

fn resolves_as_non_string(s: &str) -> bool {
    match s {
        "null" | "Null" | "NULL" | "~" | "true" | "True" | "TRUE" | "false" | "False" | "FALSE" => {
            return true
        }
        _ => {}
    }
    if is_core_int(s) || is_core_float(s) {
        return true;
    }
    false
}

fn is_core_int(s: &str) -> bool {
    let body = s.strip_prefix(['+', '-']).unwrap_or(s);
    if let Some(hex) = body.strip_prefix("0x") {
        return !hex.is_empty() && hex.chars().all(|c| c.is_ascii_hexdigit());
    }
    if let Some(oct) = body.strip_prefix("0o") {
        return !oct.is_empty() && oct.chars().all(|c| ('0'..='7').contains(&c));
    }
    !body.is_empty() && body.chars().all(|c| c.is_ascii_digit())
}

fn is_core_float(s: &str) -> bool {
    let body = s.strip_prefix(['+', '-']).unwrap_or(s);
    if matches!(body, ".inf" | ".Inf" | ".INF") {
        return true;
    }
    if matches!(s, ".nan" | ".NaN" | ".NAN") {
        return true;
    }
    // [0-9]* ( '.' [0-9]* )? ( [eE] [+-]? [0-9]+ )?  with at least one digit
    let (mantissa, exp) = match body.split_once(['e', 'E']) {
        Some((m, e)) => (m, Some(e)),
        None => (body, None),
    };
    let mantissa_ok = match mantissa.split_once('.') {
        Some((int, frac)) => {
            (int.chars().all(|c| c.is_ascii_digit()))
                && (frac.chars().all(|c| c.is_ascii_digit()))
                && !(int.is_empty() && frac.is_empty())
                && (!int.is_empty() || !frac.is_empty())
        }
        None => false, // no dot -> handled by is_core_int, not a float
    };
    if !mantissa_ok || mantissa.is_empty() {
        return false;
    }
    match exp {
        None => true,
        Some(e) => {
            let e = e.strip_prefix(['+', '-']).unwrap_or(e);
            !e.is_empty() && e.chars().all(|c| c.is_ascii_digit())
        }
    }
}

fn can_be_plain(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }
    let chars: Vec<char> = s.chars().collect();
    if chars[0].is_whitespace() || chars[chars.len() - 1].is_whitespace() {
        return false;
    }
    if s.contains('\n') || chars.iter().copied().any(is_control) {
        return false;
    }
    let c0 = chars[0];
    if FORBIDDEN_FIRST.contains(c0) {
        return false;
    }
    if matches!(c0, '-' | '?' | ':') {
        match chars.get(1) {
            None | Some(' ') => return false,
            _ => {}
        }
    }
    if s.contains(": ") || s.ends_with(':') || s.contains(" #") {
        return false;
    }
    if resolves_as_non_string(s) {
        return false;
    }
    true
}

fn double_quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            _ => out.push(c),
        }
    }
    out.push('"');
    out
}

fn single_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

fn emit_string_scalar(s: &str) -> String {
    if s.is_empty() {
        return "\"\"".to_string();
    }
    if can_be_plain(s) {
        return s.to_string();
    }
    if s.contains('"') && !s.contains('\'') {
        return single_quote(s);
    }
    double_quote(s)
}

fn emit_scalar(node: &Yaml) -> String {
    match node {
        Yaml::Null => "null".to_string(),
        Yaml::Bool(b) => b.to_string(),
        Yaml::Int(i) => i.to_string(),
        Yaml::Float(f) => f.to_string(),
        Yaml::Str(s) => emit_string_scalar(s),
        Yaml::Seq(_) | Yaml::Map(_) => unreachable!("emit_scalar called on a collection"),
    }
}

fn is_empty_collection(node: &Yaml) -> Option<&'static str> {
    match node {
        Yaml::Seq(items) if items.is_empty() => Some("[]"),
        Yaml::Map(entries) if entries.is_empty() => Some("{}"),
        _ => None,
    }
}

fn pad(indent: usize) -> String {
    " ".repeat(indent)
}

fn emit_map(entries: &[(String, Yaml)], indent: usize, out: &mut Vec<String>) {
    for (key, value) in entries {
        let key_repr = emit_string_scalar(key);
        match value {
            Yaml::Seq(items) if !items.is_empty() => {
                out.push(format!("{}{key_repr}:", pad(indent)));
                emit_seq(items, indent + 2, out);
            }
            Yaml::Map(inner) if !inner.is_empty() => {
                out.push(format!("{}{key_repr}:", pad(indent)));
                emit_map(inner, indent + 2, out);
            }
            other => {
                let scalar = is_empty_collection(other)
                    .map_or_else(|| emit_scalar(other), ToString::to_string);
                out.push(format!("{}{key_repr}: {scalar}", pad(indent)));
            }
        }
    }
}

fn emit_seq(items: &[Yaml], indent: usize, out: &mut Vec<String>) {
    for item in items {
        match item {
            Yaml::Map(inner) if !inner.is_empty() => {
                let mut block: Vec<String> = Vec::new();
                emit_map(inner, indent + 2, &mut block);
                let first = block.remove(0);
                let first_trimmed = &first[indent + 2..];
                out.push(format!("{}- {first_trimmed}", pad(indent)));
                out.extend(block);
            }
            Yaml::Seq(inner) if !inner.is_empty() => {
                out.push(format!("{}-", pad(indent)));
                emit_seq(inner, indent + 2, out);
            }
            other => {
                let scalar = is_empty_collection(other)
                    .map_or_else(|| emit_scalar(other), ToString::to_string);
                out.push(format!("{}- {scalar}", pad(indent)));
            }
        }
    }
}

/// Render a [`Yaml`] value as a document, matching `eemeli/yaml`'s block style
/// with a trailing newline.
#[must_use]
pub fn to_yaml_document(node: &Yaml) -> String {
    let mut lines: Vec<String> = Vec::new();
    match node {
        Yaml::Map(entries) if !entries.is_empty() => emit_map(entries, 0, &mut lines),
        Yaml::Seq(items) if !items.is_empty() => emit_seq(items, 0, &mut lines),
        other => {
            let scalar =
                is_empty_collection(other).map_or_else(|| emit_scalar(other), ToString::to_string);
            lines.push(scalar);
        }
    }
    let mut out = lines.join("\n");
    out.push('\n');
    out
}

fn line_range(range: crate::schemas::LineRange) -> Yaml {
    Yaml::Seq(vec![
        Yaml::Int(i64::from(range.start)),
        Yaml::Int(i64::from(range.end)),
    ])
}

fn json_to_yaml(value: &serde_json::Value) -> Yaml {
    match value {
        serde_json::Value::Null => Yaml::Null,
        serde_json::Value::Bool(b) => Yaml::Bool(*b),
        serde_json::Value::Number(n) => n
            .as_i64()
            .map_or_else(|| Yaml::Float(n.as_f64().unwrap_or(0.0)), Yaml::Int),
        serde_json::Value::String(s) => Yaml::Str(s.clone()),
        serde_json::Value::Array(items) => Yaml::Seq(items.iter().map(json_to_yaml).collect()),
        serde_json::Value::Object(map) => Yaml::Map(
            map.iter()
                .map(|(k, v)| (k.clone(), json_to_yaml(v)))
                .collect(),
        ),
    }
}

/// Serialize an [`Attribution`] the way the CLI does (`stringifyYaml`).
#[must_use]
pub fn attribution_to_yaml(attribution: &Attribution) -> String {
    let entries = attribution
        .entries
        .iter()
        .map(|entry| {
            Yaml::Map(vec![
                ("promptLines".to_string(), line_range(entry.prompt_lines)),
                ("file".to_string(), Yaml::Str(entry.file.clone())),
                ("lines".to_string(), line_range(entry.lines)),
                ("note".to_string(), Yaml::Str(entry.note.clone())),
            ])
        })
        .collect();
    let doc = Yaml::Map(vec![
        ("module".to_string(), Yaml::Str(attribution.module.clone())),
        ("target".to_string(), Yaml::Str(attribution.target.clone())),
        ("entries".to_string(), Yaml::Seq(entries)),
    ]);
    to_yaml_document(&doc)
}

fn ml_kind_str(kind: MlKind) -> &'static str {
    match kind {
        MlKind::Ambiguity => "ambiguity",
        MlKind::Assumption => "assumption",
        MlKind::Note => "note",
        MlKind::NoOp => "no-op",
    }
}

/// Serialize a machine-layer document the way the CLI does (`stringifyYaml`).
#[must_use]
pub fn ml_to_yaml(ml: &Ml) -> String {
    let entries = ml
        .entries
        .iter()
        .map(|entry| {
            let mut fields = vec![
                ("promptLines".to_string(), line_range(entry.prompt_lines)),
                (
                    "kind".to_string(),
                    Yaml::Str(ml_kind_str(entry.kind).to_string()),
                ),
                ("message".to_string(), Yaml::Str(entry.message.clone())),
                ("reasoning".to_string(), Yaml::Str(entry.reasoning.clone())),
            ];
            if let Some(suggestion) = &entry.suggestion {
                fields.push(("suggestion".to_string(), Yaml::Str(suggestion.clone())));
            }
            Yaml::Map(fields)
        })
        .collect();
    let doc = Yaml::Map(vec![
        ("module".to_string(), Yaml::Str(ml.module.clone())),
        ("target".to_string(), Yaml::Str(ml.target.clone())),
        ("entries".to_string(), Yaml::Seq(entries)),
    ]);
    to_yaml_document(&doc)
}

/// Serialize an [`Ir`] document the way the CLI does (`stringifyYaml`).
#[must_use]
pub fn ir_to_yaml(ir: &Ir) -> String {
    let types = ir
        .types
        .iter()
        .map(|t| {
            Yaml::Map(vec![
                ("name".to_string(), Yaml::Str(t.name.clone())),
                ("description".to_string(), Yaml::Str(t.description.clone())),
            ])
        })
        .collect();
    let functions = ir
        .functions
        .iter()
        .map(|f| {
            Yaml::Map(vec![
                ("name".to_string(), Yaml::Str(f.name.clone())),
                ("signature".to_string(), Yaml::Str(f.signature.clone())),
                ("behavior".to_string(), Yaml::Str(f.behavior.clone())),
            ])
        })
        .collect();
    let tests = ir
        .tests
        .iter()
        .map(|t| {
            Yaml::Map(vec![
                ("name".to_string(), Yaml::Str(t.name.clone())),
                ("given".to_string(), json_to_yaml(&t.given)),
                ("expect".to_string(), json_to_yaml(&t.expect)),
            ])
        })
        .collect();
    let doc = Yaml::Map(vec![
        ("module".to_string(), Yaml::Str(ir.module.clone())),
        (
            "deps".to_string(),
            Yaml::Seq(ir.deps.iter().map(|d| Yaml::Str(d.clone())).collect()),
        ),
        ("types".to_string(), Yaml::Seq(types)),
        ("functions".to_string(), Yaml::Seq(functions)),
        ("tests".to_string(), Yaml::Seq(tests)),
    ]);
    to_yaml_document(&doc)
}

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
