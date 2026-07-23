//! Machine-layer (ml/mapl) schema: model annotations keyed to prompt lines.

use serde::Deserialize;

use super::line_range::LineRange;
use super::{require_non_empty, SchemaError};

/// The four annotation kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MlKind {
    Ambiguity,
    Assumption,
    Note,
    #[serde(rename = "no-op")]
    NoOp,
}

/// One machine-layer annotation entry.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct MlEntry {
    #[serde(rename = "promptLines")]
    pub prompt_lines: LineRange,
    pub kind: MlKind,
    pub message: String,
    #[serde(default)]
    pub reasoning: String,
    #[serde(default)]
    pub suggestion: Option<String>,
}

/// A machine-layer document for one module/target pair.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Ml {
    pub module: String,
    pub target: String,
    #[serde(default)]
    pub entries: Vec<MlEntry>,
}

fn validate_entry(entry: &MlEntry) -> Result<(), SchemaError> {
    require_non_empty(&entry.message, "ml entry message")
}

/// Validate a machine-layer document, mirroring `validateMl`.
pub fn validate_ml(value: serde_json::Value) -> Result<Ml, SchemaError> {
    let ml: Ml =
        serde_json::from_value(value).map_err(|e| SchemaError::Deserialize(e.to_string()))?;
    require_non_empty(&ml.module, "module")?;
    require_non_empty(&ml.target, "target")?;
    for entry in &ml.entries {
        validate_entry(entry)?;
    }
    Ok(ml)
}

/// Parse a list of ml entries, treating a non-list as empty, mirroring
/// `parseMlEntries`.
pub fn parse_ml_entries(value: serde_json::Value) -> Result<Vec<MlEntry>, SchemaError> {
    if !value.is_array() {
        return Ok(Vec::new());
    }
    let entries: Vec<MlEntry> =
        serde_json::from_value(value).map_err(|e| SchemaError::Deserialize(e.to_string()))?;
    for entry in &entries {
        validate_entry(entry)?;
    }
    Ok(entries)
}

/// Return every entry whose range covers `body_line`.
#[must_use]
pub fn ml_entries_at_body_line(ml: &Ml, body_line: u32) -> Vec<&MlEntry> {
    ml.entries
        .iter()
        .filter(|entry| {
            body_line >= entry.prompt_lines.start && body_line <= entry.prompt_lines.end
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn accepts_valid_document_with_normalized_range() {
        let ml = validate_ml(json!({
            "module": "todo-app",
            "target": "react",
            "entries": [
                { "promptLines": 18, "kind": "ambiguity", "message": "odd phrase", "reasoning": "unclear" }
            ]
        }))
        .unwrap();
        assert_eq!(ml.entries[0].prompt_lines, LineRange::new(18, 18));
        assert_eq!(ml.entries[0].kind, MlKind::Ambiguity);
        assert_eq!(ml.entries[0].suggestion, None);
    }

    #[test]
    fn defaults_empty_entries() {
        let ml = validate_ml(json!({ "module": "m", "target": "react" })).unwrap();
        assert!(ml.entries.is_empty());
    }

    #[test]
    fn accepts_all_kinds_including_no_op() {
        for kind in ["ambiguity", "assumption", "note", "no-op"] {
            let ml = validate_ml(json!({
                "module": "m", "target": "react",
                "entries": [{ "promptLines": [1, 1], "kind": kind, "message": "x" }]
            }))
            .unwrap();
            assert_eq!(ml.entries.len(), 1);
        }
    }

    #[test]
    fn rejects_unknown_kind() {
        assert!(validate_ml(json!({
            "module": "m", "target": "react",
            "entries": [{ "promptLines": [1, 1], "kind": "bogus", "message": "x" }]
        }))
        .is_err());
    }

    #[test]
    fn rejects_empty_message() {
        assert!(validate_ml(json!({
            "module": "m", "target": "react",
            "entries": [{ "promptLines": [1, 1], "kind": "note", "message": "" }]
        }))
        .is_err());
    }

    #[test]
    fn parse_entries_list_and_non_list() {
        assert_eq!(
            parse_ml_entries(json!([
                { "promptLines": [2, 3], "kind": "assumption", "message": "a" }
            ]))
            .unwrap()
            .len(),
            1
        );
        assert!(parse_ml_entries(json!({})).unwrap().is_empty());
    }

    #[test]
    fn parse_entries_throws_on_malformed() {
        assert!(parse_ml_entries(json!([
            { "promptLines": "nope", "kind": "note", "message": "x" }
        ]))
        .is_err());
    }

    #[test]
    fn entries_at_body_line() {
        let ml = validate_ml(json!({
            "module": "m", "target": "react",
            "entries": [
                { "promptLines": [1, 2], "kind": "note", "message": "a" },
                { "promptLines": [5, 7], "kind": "ambiguity", "message": "b" }
            ]
        }))
        .unwrap();
        assert_eq!(
            ml_entries_at_body_line(&ml, 6)
                .iter()
                .map(|e| e.message.as_str())
                .collect::<Vec<_>>(),
            vec!["b"]
        );
        assert_eq!(
            ml_entries_at_body_line(&ml, 1)
                .iter()
                .map(|e| e.message.as_str())
                .collect::<Vec<_>>(),
            vec!["a"]
        );
        assert!(ml_entries_at_body_line(&ml, 4).is_empty());
    }
}
