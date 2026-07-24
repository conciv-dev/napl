//! Stage1 adapter over the NAPL-generated `schemas_attribution` crate. The
//! generated crate surfaces its own `AttributionError`; the wrappers map it to
//! the shared `SchemaError` the callers expect.

use super::SchemaError;

pub use schemas_attribution::{entries_at_body_line, Attribution, AttributionEntry};

#[cfg(test)]
use super::LineRange;

pub fn validate_attribution(value: serde_json::Value) -> Result<Attribution, SchemaError> {
    schemas_attribution::validate_attribution(value)
        .map_err(|e| SchemaError::Deserialize(e.to_string()))
}

pub fn parse_attribution_entries(
    value: serde_json::Value,
) -> Result<Vec<AttributionEntry>, SchemaError> {
    schemas_attribution::parse_attribution_entries(value)
        .map_err(|e| SchemaError::Deserialize(e.to_string()))
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
