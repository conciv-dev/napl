//! Reverse-navigation pure helpers: mapping a generated source location back to
//! the prompt sentence(s) that produced it. Ported from the TypeScript LSP
//! `reverse.ts`; all functions are side-effect-free and operate on already-loaded
//! attribution data.

use crate::body_lines::PromptBody;
use crate::schemas::{AttributionEntry, LineRange};

/// The `.napl/src/` prefix every generated file path carries.
pub const GENERATED_PREFIX: &str = ".napl/src/";

/// The CodeLens banner shown when a generated file has drifted from its map hash.
pub const DRIFT_LENS_PREFIX: &str = "DRIFT — edits here are not reflected in any prompt";

/// A generated file split into its target and target-relative path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneratedPathInfo {
    pub target: String,
    pub target_rel_path: String,
}

/// Split a repo-relative generated path (`.napl/src/<target>/<rest>`) into its
/// target and target-relative path, or `None` when it is not a generated file.
#[must_use]
pub fn parse_generated_path(rel_full: &str) -> Option<GeneratedPathInfo> {
    let normalized = rel_full.replace('\\', "/");
    let rest = normalized.strip_prefix(GENERATED_PREFIX)?;
    let slash = rest.find('/')?;
    if slash == 0 {
        return None;
    }
    let target = &rest[..slash];
    let target_rel_path = &rest[slash + 1..];
    if target_rel_path.is_empty() {
        return None;
    }
    Some(GeneratedPathInfo {
        target: target.to_string(),
        target_rel_path: target_rel_path.to_string(),
    })
}

/// One module's attribution together with the prompt files that contribute to it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttributionSource {
    pub module: String,
    pub target: String,
    pub entries: Vec<AttributionEntry>,
    pub prompt_files: Vec<String>,
}

/// A single reverse hit: a generated code range and the prompt span behind it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReverseMatch {
    pub module: String,
    pub target: String,
    pub prompt_file: String,
    pub note: String,
    pub prompt_lines: LineRange,
    pub code_lines: LineRange,
}

fn in_range(value: u32, range: LineRange) -> bool {
    value >= range.start && value <= range.end
}

/// Every reverse match for a generated file line; a `None` `code_line` returns
/// all entries for the file regardless of line.
#[must_use]
pub fn reverse_matches(
    sources: &[AttributionSource],
    target: &str,
    target_rel_path: &str,
    code_line: Option<u32>,
) -> Vec<ReverseMatch> {
    let mut matches = Vec::new();
    for source in sources {
        if source.target != target {
            continue;
        }
        for entry in &source.entries {
            if entry.file != target_rel_path {
                continue;
            }
            if let Some(line) = code_line {
                if !in_range(line, entry.lines) {
                    continue;
                }
            }
            for prompt_file in &source.prompt_files {
                matches.push(ReverseMatch {
                    module: source.module.clone(),
                    target: source.target.clone(),
                    prompt_file: prompt_file.clone(),
                    note: entry.note.clone(),
                    prompt_lines: entry.prompt_lines,
                    code_lines: entry.lines,
                });
            }
        }
    }
    matches
}

/// Convert body-relative 1-based prompt lines to absolute 0-based document lines.
#[must_use]
pub fn prompt_absolute_lines(body_start_line: usize, prompt_lines: LineRange) -> (usize, usize) {
    (
        body_start_line + prompt_lines.start as usize - 1,
        body_start_line + prompt_lines.end as usize - 1,
    )
}

/// The absolute 0-based document lines of a match's prompt span within `body`.
#[must_use]
pub fn match_prompt_lines(body: &PromptBody, prompt_lines: LineRange) -> (usize, usize) {
    prompt_absolute_lines(body.body_start_line, prompt_lines)
}

/// Whether a recorded hash differs from the actual on-disk hash (drift).
#[must_use]
pub fn is_file_drifted(recorded_hash: Option<&str>, actual_hash: &str) -> bool {
    match recorded_hash {
        Some(hash) => hash != actual_hash,
        None => false,
    }
}

/// The CodeLens title pointing back at a prompt sentence.
#[must_use]
pub fn code_lens_title(prompt_basename: &str, absolute_line: usize, note: &str) -> String {
    if note.is_empty() {
        format!("⇠ {prompt_basename}:{absolute_line}")
    } else {
        format!("⇠ {prompt_basename}:{absolute_line} — {note}")
    }
}

/// Collapse matches that resolve to the same prompt span.
#[must_use]
pub fn dedupe_matches(matches: &[ReverseMatch]) -> Vec<ReverseMatch> {
    let mut seen = std::collections::HashSet::new();
    let mut result = Vec::new();
    for m in matches {
        let key = format!(
            "{}#{}:{}",
            m.prompt_file, m.prompt_lines.start, m.prompt_lines.end
        );
        if seen.insert(key) {
            result.push(m.clone());
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(pl: (u32, u32), file: &str, cl: (u32, u32), note: &str) -> AttributionEntry {
        AttributionEntry {
            prompt_lines: LineRange::new(pl.0, pl.1),
            file: file.to_string(),
            lines: LineRange::new(cl.0, cl.1),
            note: note.to_string(),
        }
    }

    fn sources() -> Vec<AttributionSource> {
        vec![AttributionSource {
            module: "greeting".to_string(),
            target: "typescript".to_string(),
            prompt_files: vec!["examples/greeting.napl".to_string()],
            entries: vec![
                entry((3, 4), "src/greeting.ts", (1, 1), "greet function signature"),
                entry((7, 7), "src/greeting.ts", (2, 2), "trims name whitespace"),
                entry((8, 8), "src/greeting.ts", (3, 5), "rejects empty name"),
                entry((6, 6), "src/greeting.ts", (6, 6), "builds greeting message"),
                entry((7, 7), "src/greeting.test.ts", (9, 11), "test trimming"),
            ],
        }]
    }

    #[test]
    fn splits_generated_path() {
        assert_eq!(
            parse_generated_path(".napl/src/typescript/src/greeting.ts"),
            Some(GeneratedPathInfo {
                target: "typescript".to_string(),
                target_rel_path: "src/greeting.ts".to_string(),
            })
        );
    }

    #[test]
    fn normalizes_windows_separators() {
        assert_eq!(
            parse_generated_path(".napl\\src\\typescript\\src\\greeting.ts"),
            Some(GeneratedPathInfo {
                target: "typescript".to_string(),
                target_rel_path: "src/greeting.ts".to_string(),
            })
        );
    }

    #[test]
    fn returns_none_for_non_generated_paths() {
        assert_eq!(parse_generated_path("examples/greeting.napl"), None);
        assert_eq!(parse_generated_path(".napl/src/typescript"), None);
        assert_eq!(parse_generated_path("src/greeting.ts"), None);
    }

    #[test]
    fn finds_entry_whose_range_contains_line() {
        let matches = reverse_matches(&sources(), "typescript", "src/greeting.ts", Some(2));
        assert_eq!(
            matches.iter().map(|m| m.note.as_str()).collect::<Vec<_>>(),
            vec!["trims name whitespace"]
        );
        assert_eq!(matches[0].prompt_lines, LineRange::new(7, 7));
        assert_eq!(matches[0].prompt_file, "examples/greeting.napl");
    }

    #[test]
    fn matches_multiline_code_range() {
        let matches = reverse_matches(&sources(), "typescript", "src/greeting.ts", Some(4));
        assert_eq!(
            matches.iter().map(|m| m.note.as_str()).collect::<Vec<_>>(),
            vec!["rejects empty name"]
        );
    }

    #[test]
    fn returns_nothing_when_target_or_file_differs() {
        assert!(reverse_matches(&sources(), "swift", "src/greeting.ts", Some(2)).is_empty());
        assert!(reverse_matches(&sources(), "typescript", "src/other.ts", Some(2)).is_empty());
    }

    #[test]
    fn returns_all_entries_when_code_line_null() {
        let matches = reverse_matches(&sources(), "typescript", "src/greeting.ts", None);
        assert_eq!(matches.len(), 4);
    }

    #[test]
    fn merges_multiple_prompts_into_separate_matches() {
        let multi = vec![AttributionSource {
            module: "greeting".to_string(),
            target: "typescript".to_string(),
            prompt_files: vec!["examples/a.napl".to_string(), "examples/b.napl".to_string()],
            entries: vec![entry((1, 1), "src/x.ts", (4, 4), "shared")],
        }];
        let matches = reverse_matches(&multi, "typescript", "src/x.ts", Some(4));
        assert_eq!(
            matches
                .iter()
                .map(|m| m.prompt_file.as_str())
                .collect::<Vec<_>>(),
            vec!["examples/a.napl", "examples/b.napl"]
        );
    }

    #[test]
    fn converts_body_relative_prompt_lines() {
        assert_eq!(prompt_absolute_lines(12, LineRange::new(7, 7)), (18, 18));
        assert_eq!(prompt_absolute_lines(12, LineRange::new(3, 4)), (14, 15));
        assert_eq!(prompt_absolute_lines(3, LineRange::new(1, 1)), (3, 3));
        assert_eq!(prompt_absolute_lines(3, LineRange::new(2, 2)), (4, 4));
    }

    #[test]
    fn drift_detection() {
        assert!(is_file_drifted(Some("aaa"), "bbb"));
        assert!(!is_file_drifted(Some("aaa"), "aaa"));
        assert!(!is_file_drifted(None, "bbb"));
    }

    #[test]
    fn lens_title_formatting() {
        assert_eq!(
            code_lens_title("greeting.napl", 19, "trims name whitespace"),
            "⇠ greeting.napl:19 — trims name whitespace"
        );
        assert_eq!(
            code_lens_title("greeting.napl", 19, ""),
            "⇠ greeting.napl:19"
        );
    }

    #[test]
    fn dedupe_collapses_duplicate_prompt_spans() {
        let matches = reverse_matches(&sources(), "typescript", "src/greeting.ts", None);
        let mut with_dupes = matches.clone();
        with_dupes.extend(matches.clone());
        assert_eq!(dedupe_matches(&with_dupes).len(), matches.len());
    }
}
