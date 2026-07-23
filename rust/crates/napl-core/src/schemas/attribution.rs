//! Attribution schema: prompt-line to file-line mappings.

use serde::Deserialize;

use super::line_range::LineRange;
use super::{require_non_empty, SchemaError};

/// One prompt-lines to file-lines mapping entry.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct AttributionEntry {
    #[serde(rename = "promptLines")]
    pub prompt_lines: LineRange,
    pub file: String,
    pub lines: LineRange,
    #[serde(default)]
    pub note: String,
}

/// A full attribution document for one module/target pair.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Attribution {
    pub module: String,
    pub target: String,
    #[serde(default)]
    pub entries: Vec<AttributionEntry>,
}

fn validate_entry(entry: &AttributionEntry) -> Result<(), SchemaError> {
    require_non_empty(&entry.file, "attribution entry file")
}

/// Validate an attribution document, mirroring `validateAttribution`.
pub fn validate_attribution(value: serde_json::Value) -> Result<Attribution, SchemaError> {
    let attribution: Attribution =
        serde_json::from_value(value).map_err(|e| SchemaError::Deserialize(e.to_string()))?;
    require_non_empty(&attribution.module, "module")?;
    require_non_empty(&attribution.target, "target")?;
    for entry in &attribution.entries {
        validate_entry(entry)?;
    }
    Ok(attribution)
}

/// Parse a list of attribution entries, treating a non-list as empty, mirroring
/// `parseAttributionEntries`.
pub fn parse_attribution_entries(
    value: serde_json::Value,
) -> Result<Vec<AttributionEntry>, SchemaError> {
    if !value.is_array() {
        return Ok(Vec::new());
    }
    let entries: Vec<AttributionEntry> =
        serde_json::from_value(value).map_err(|e| SchemaError::Deserialize(e.to_string()))?;
    for entry in &entries {
        validate_entry(entry)?;
    }
    Ok(entries)
}

/// Return every entry whose prompt range contains `body_line`.
#[must_use]
pub fn entries_at_body_line(attribution: &Attribution, body_line: u32) -> Vec<&AttributionEntry> {
    attribution
        .entries
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

    fn sample() -> Attribution {
        validate_attribution(json!({
            "module": "greeting",
            "target": "typescript",
            "entries": [
                { "promptLines": [2, 2], "file": "greeting.ts", "lines": [1, 3], "note": "builds the greeting" },
                { "promptLines": [3, 4], "file": "greeting.ts", "lines": [5, 7], "note": "trims whitespace" },
                { "promptLines": [3, 3], "file": "greeting.test.ts", "lines": [10, 12], "note": "covers trimming" }
            ]
        }))
        .unwrap()
    }

    #[test]
    fn validates_well_formed_document() {
        assert_eq!(sample().entries.len(), 3);
    }

    #[test]
    fn rejects_missing_file() {
        let err = validate_attribution(json!({
            "module": "m", "target": "t",
            "entries": [{ "promptLines": [1, 1], "lines": [1, 1], "note": "x" }]
        }));
        assert!(err.is_err());
    }

    #[test]
    fn rejects_empty_file() {
        let err = validate_attribution(json!({
            "module": "m", "target": "t",
            "entries": [{ "promptLines": [1, 1], "file": "", "lines": [1, 1] }]
        }));
        assert!(err.is_err());
    }

    #[test]
    fn rejects_non_integer_line_range() {
        let err = validate_attribution(json!({
            "module": "m", "target": "t",
            "entries": [{ "promptLines": [1, 1], "file": "a.ts", "lines": [1.5, 2], "note": "x" }]
        }));
        assert!(err.is_err());
    }

    #[test]
    fn normalizes_single_line_range() {
        let entries = parse_attribution_entries(json!([
            { "promptLines": [8], "file": "a.ts", "lines": 3, "note": "single line" }
        ]))
        .unwrap();
        assert_eq!(entries[0].prompt_lines, LineRange::new(8, 8));
        assert_eq!(entries[0].lines, LineRange::new(3, 3));
    }

    #[test]
    fn default_note_is_empty() {
        let entries = parse_attribution_entries(json!([
            { "promptLines": [1, 1], "file": "a.ts", "lines": [1, 1] }
        ]))
        .unwrap();
        assert_eq!(entries[0].note, "");
    }

    #[test]
    fn parse_entries_non_list_is_empty() {
        assert!(parse_attribution_entries(json!({})).unwrap().is_empty());
    }

    #[test]
    fn throws_on_malformed_list() {
        assert!(parse_attribution_entries(json!([
            { "promptLines": "nope", "file": "a", "lines": [1, 2] }
        ]))
        .is_err());
    }

    #[test]
    fn entries_at_body_line_range_lookup() {
        let attribution = sample();
        let mut at_three: Vec<&str> = entries_at_body_line(&attribution, 3)
            .iter()
            .map(|e| e.note.as_str())
            .collect();
        at_three.sort_unstable();
        assert_eq!(at_three, vec!["covers trimming", "trims whitespace"]);

        assert_eq!(
            entries_at_body_line(&attribution, 2)
                .iter()
                .map(|e| e.note.as_str())
                .collect::<Vec<_>>(),
            vec!["builds the greeting"]
        );
        assert!(entries_at_body_line(&attribution, 9).is_empty());
    }
}
