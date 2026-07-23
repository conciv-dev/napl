//! Journal (JSONL) schema, its corrupt-line-skipping reader, and helpers.

use serde::{Deserialize, Serialize};

use crate::blame::BlameSourceEntry;
use crate::text_diff::unified_diff;

use super::SchemaError;

/// The per-file record inside a journal entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JournalFile {
    pub path: String,
    pub patch: String,
    #[serde(rename = "hashBefore")]
    pub hash_before: Option<String>,
    #[serde(rename = "hashAfter")]
    pub hash_after: String,
}

/// The generation mode of a journal entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum JournalMode {
    Full,
    Incremental,
}

/// One journal entry (one generation).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JournalEntry {
    pub gen: i64,
    pub timestamp: String,
    pub module: String,
    pub target: String,
    #[serde(rename = "promptHash")]
    pub prompt_hash: String,
    #[serde(rename = "promptDiff")]
    pub prompt_diff: String,
    pub mode: JournalMode,
    #[serde(default)]
    pub files: Vec<JournalFile>,
}

fn validate_entry(entry: &JournalEntry) -> Result<(), SchemaError> {
    if entry.gen < 1 {
        return Err(SchemaError::Validation("gen must be >= 1".to_string()));
    }
    if entry.module.is_empty() {
        return Err(SchemaError::Validation(
            "module must not be empty".to_string(),
        ));
    }
    if entry.target.is_empty() {
        return Err(SchemaError::Validation(
            "target must not be empty".to_string(),
        ));
    }
    for file in &entry.files {
        if file.path.is_empty() {
            return Err(SchemaError::Validation(
                "file path must not be empty".to_string(),
            ));
        }
    }
    Ok(())
}

/// The unified diff for a created (`before == None`) or modified file.
#[must_use]
pub fn file_patch(before: Option<&str>, after: &str) -> String {
    unified_diff(before.unwrap_or(""), after)
}

/// Read a journal from its raw JSONL text, skipping corrupt (unparseable) and
/// schema-invalid lines. Returns the valid entries and a warning per skip,
/// mirroring `readJournal`.
#[must_use]
pub fn read_journal_str(raw: &str) -> (Vec<JournalEntry>, Vec<String>) {
    let mut entries = Vec::new();
    let mut warnings = Vec::new();
    for (index, line) in raw
        .split('\n')
        .map(|s| s.strip_suffix('\r').unwrap_or(s))
        .enumerate()
    {
        if line.trim().is_empty() {
            continue;
        }
        let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
            warnings.push(format!(
                "journal: skipping corrupt line {} (invalid JSON)",
                index + 1
            ));
            continue;
        };
        match serde_json::from_value::<JournalEntry>(value).and_then(|entry| {
            validate_entry(&entry).map_err(serde::de::Error::custom)?;
            Ok(entry)
        }) {
            Ok(entry) => entries.push(entry),
            Err(err) => warnings.push(format!(
                "journal: skipping corrupt line {} ({err})",
                index + 1
            )),
        }
    }
    (entries, warnings)
}

/// One past the highest recorded gen, or 1 for an empty journal.
#[must_use]
pub fn next_gen_number(entries: &[JournalEntry]) -> i64 {
    entries.iter().map(|e| e.gen).max().unwrap_or(0) + 1
}

/// The blame source history for a single file across the journal.
#[must_use]
pub fn file_history(entries: &[JournalEntry], file_path: &str) -> Vec<BlameSourceEntry> {
    let mut history = Vec::new();
    for entry in entries {
        if let Some(file) = entry.files.iter().find(|f| f.path == file_path) {
            history.push(BlameSourceEntry {
                gen: entry.gen,
                timestamp: entry.timestamp.clone(),
                module: entry.module.clone(),
                patch: file.patch.clone(),
                prompt_diff: entry.prompt_diff.clone(),
            });
        }
    }
    history
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(gen: i64, path: &str) -> JournalEntry {
        JournalEntry {
            gen,
            timestamp: format!("t{gen}"),
            module: "demo".to_string(),
            target: "react".to_string(),
            prompt_hash: format!("h{gen}"),
            prompt_diff: String::new(),
            mode: JournalMode::Full,
            files: vec![JournalFile {
                path: path.to_string(),
                patch: file_patch(None, "x\n"),
                hash_before: None,
                hash_after: "abc".to_string(),
            }],
        }
    }

    #[test]
    fn file_patch_created_file() {
        let patch = file_patch(None, "a\nb\n");
        assert!(patch.contains("@@ -0,0 +1,2 @@"));
        assert!(patch.contains("+a"));
        assert!(patch.contains("+b"));
    }

    #[test]
    fn file_patch_modified_file() {
        let patch = file_patch(Some("a\nb\nc\n"), "a\nB\nc\n");
        assert!(patch.contains("-b"));
        assert!(patch.contains("+B"));
    }

    #[test]
    fn round_trips_appended_entries() {
        let raw = format!(
            "{}\n{}\n",
            serde_json::to_string(&entry(1, "f.ts")).unwrap(),
            serde_json::to_string(&entry(2, "f.ts")).unwrap()
        );
        let (entries, warnings) = read_journal_str(&raw);
        assert_eq!(
            entries.iter().map(|e| e.gen).collect::<Vec<_>>(),
            vec![1, 2]
        );
        assert!(warnings.is_empty());
    }

    #[test]
    fn empty_journal_is_empty() {
        let (entries, _) = read_journal_str("");
        assert!(entries.is_empty());
    }

    #[test]
    fn skips_corrupt_and_invalid_lines_with_warnings() {
        let valid = serde_json::to_string(&entry(1, "f.ts")).unwrap();
        let invalid_schema = r#"{"gen":"nope","module":"x"}"#;
        let raw = format!("{valid}\nnot json at all\n{invalid_schema}\n");
        let (entries, warnings) = read_journal_str(&raw);
        assert_eq!(entries.iter().map(|e| e.gen).collect::<Vec<_>>(), vec![1]);
        assert_eq!(warnings.len(), 2);
    }

    #[test]
    fn next_gen_number_cases() {
        assert_eq!(next_gen_number(&[]), 1);
        assert_eq!(
            next_gen_number(&[entry(3, "a"), entry(7, "b"), entry(5, "c")]),
            8
        );
    }

    #[test]
    fn file_history_filters_and_carries_patch() {
        let entries = [entry(1, "a.ts"), entry(2, "b.ts"), entry(3, "a.ts")];
        let history = file_history(&entries, "a.ts");
        assert_eq!(
            history.iter().map(|h| h.gen).collect::<Vec<_>>(),
            vec![1, 3]
        );
        assert!(history[0].patch.contains("+x"));
    }

    #[test]
    fn file_history_empty_for_missing_file() {
        assert!(file_history(&[entry(1, "a.ts")], "missing.ts").is_empty());
    }
}
