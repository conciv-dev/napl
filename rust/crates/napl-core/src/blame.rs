//! Patch-replay line ancestry ("blame"), matching the TS `blame` semantics
//! exactly: added / modified / moved-by-insertion / created-then-edited /
//! untouched-keeps-oldest-gen.

use std::collections::HashMap;

use crate::text_diff::{parse_hunks, to_lines, HunkKind};

/// One recorded generation touching a file, feeding [`blame_file`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlameSourceEntry {
    pub gen: i64,
    pub timestamp: String,
    pub module: String,
    pub patch: String,
    pub prompt_diff: String,
}

/// A single blamed line of the current file content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlameLine {
    pub line: usize,
    pub gen: i64,
    pub timestamp: String,
    pub module: String,
    pub text: String,
}

/// Replay a patch against a per-line generation vector, tagging inserted and
/// unknown-context lines with `gen`.
#[must_use]
pub fn apply_patch_to_blame(blame: &[i64], patch: &str, gen: i64) -> Vec<i64> {
    let hunks = parse_hunks(patch);
    if hunks.is_empty() {
        return blame.to_vec();
    }
    let mut result: Vec<i64> = Vec::new();
    let mut old_idx = 0usize;
    for hunk in &hunks {
        let copy_until = hunk.old_start.saturating_sub(1);
        while old_idx < copy_until && old_idx < blame.len() {
            result.push(blame[old_idx]);
            old_idx += 1;
        }
        for line in &hunk.lines {
            match line.kind {
                HunkKind::Context => {
                    result.push(blame.get(old_idx).copied().unwrap_or(gen));
                    old_idx += 1;
                }
                HunkKind::Del => {
                    old_idx += 1;
                }
                HunkKind::Ins => {
                    result.push(gen);
                }
            }
        }
    }
    while old_idx < blame.len() {
        result.push(blame[old_idx]);
        old_idx += 1;
    }
    result
}

/// Blame every line of `current_content` against the file's `history`.
#[must_use]
pub fn blame_file(history: &[BlameSourceEntry], current_content: &str) -> Vec<BlameLine> {
    let mut ordered = history.to_vec();
    ordered.sort_by_key(|entry| entry.gen);

    let mut blame: Vec<i64> = Vec::new();
    for entry in &ordered {
        blame = apply_patch_to_blame(&blame, &entry.patch, entry.gen);
    }

    let mut by_gen: HashMap<i64, &BlameSourceEntry> = HashMap::new();
    for entry in &ordered {
        by_gen.insert(entry.gen, entry);
    }
    let fallback_gen = ordered.last().map_or(0, |entry| entry.gen);

    to_lines(current_content)
        .into_iter()
        .enumerate()
        .map(|(index, text)| {
            let gen = blame.get(index).copied().unwrap_or(fallback_gen);
            let entry = by_gen.get(&gen);
            BlameLine {
                line: index + 1,
                gen,
                timestamp: entry.map_or_else(String::new, |e| e.timestamp.clone()),
                module: entry.map_or_else(String::new, |e| e.module.clone()),
                text,
            }
        })
        .collect()
}

/// Blame a single 1-based line, or `None` when out of range.
#[must_use]
pub fn blame_line_at(
    history: &[BlameSourceEntry],
    current_content: &str,
    line: usize,
) -> Option<BlameLine> {
    blame_file(history, current_content)
        .into_iter()
        .find(|entry| entry.line == line)
}

/// The most informative single line of a prompt diff: the first added line,
/// else the first removed line, else empty.
#[must_use]
pub fn first_prompt_diff_line(prompt_diff: &str) -> String {
    let mut seen_header = false;
    let mut first_removal = String::new();
    for line in prompt_diff
        .split('\n')
        .map(|s| s.strip_suffix('\r').unwrap_or(s))
    {
        if line.starts_with("@@") {
            seen_header = true;
            continue;
        }
        if !seen_header {
            continue;
        }
        let text = line
            .char_indices()
            .nth(1)
            .map_or("", |(idx, _)| &line[idx..])
            .trim();
        if text.is_empty() {
            continue;
        }
        if line.starts_with('+') {
            return text.to_string();
        }
        if line.starts_with('-') && first_removal.is_empty() {
            first_removal = text.to_string();
        }
    }
    first_removal
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text_diff::unified_diff;

    fn file_patch(before: Option<&str>, after: &str) -> String {
        unified_diff(before.unwrap_or(""), after)
    }

    struct Version {
        gen: i64,
        content: &'static str,
        module: &'static str,
        timestamp: String,
    }

    fn history(versions: &[Version]) -> Vec<BlameSourceEntry> {
        let mut entries = Vec::new();
        let mut prev: Option<&str> = None;
        for v in versions {
            entries.push(BlameSourceEntry {
                gen: v.gen,
                timestamp: v.timestamp.clone(),
                module: v.module.to_string(),
                patch: file_patch(prev, v.content),
                prompt_diff: String::new(),
            });
            prev = Some(v.content);
        }
        entries
    }

    fn ver(gen: i64, content: &'static str) -> Version {
        Version {
            gen,
            content,
            module: "demo",
            timestamp: format!("2026-07-2{gen}T00:00:00.000Z"),
        }
    }

    fn gens(entries: &[BlameSourceEntry], content: &str) -> Vec<i64> {
        blame_file(entries, content)
            .into_iter()
            .map(|line| line.gen)
            .collect()
    }

    #[test]
    fn single_gen_file_all_lines_that_gen() {
        let content = "A\nB\nC\n";
        let entries = history(&[ver(1, content)]);
        assert_eq!(gens(&entries, content), vec![1, 1, 1]);
    }

    #[test]
    fn untouched_stays_old_modified_moves() {
        let entries = history(&[ver(1, "A\nB\nC\n"), ver(3, "A\nB2\nC\n")]);
        assert_eq!(gens(&entries, "A\nB2\nC\n"), vec![1, 3, 1]);
    }

    #[test]
    fn appended_line_attributed_to_adding_gen() {
        let entries = history(&[ver(1, "A\n"), ver(2, "A\nB\n")]);
        assert_eq!(gens(&entries, "A\nB\n"), vec![1, 2]);
    }

    #[test]
    fn line_moved_down_by_insertion_above() {
        let entries = history(&[ver(1, "A\nB\n"), ver(2, "X\nA\nB\n")]);
        assert_eq!(gens(&entries, "X\nA\nB\n"), vec![2, 1, 1]);
    }

    #[test]
    fn created_then_edited_across_multiple_hunks() {
        let entries = history(&[ver(1, "a\nb\nc\nd\ne\n"), ver(3, "a\nB\nc\nd\nE\nf\n")]);
        assert_eq!(gens(&entries, "a\nB\nc\nd\nE\nf\n"), vec![1, 3, 1, 1, 3, 3]);
    }

    #[test]
    fn carries_timestamp_and_module_from_attributing_gen() {
        let versions = [
            Version {
                gen: 1,
                content: "A\nB\n",
                module: "first",
                timestamp: "ts1".to_string(),
            },
            Version {
                gen: 4,
                content: "A\nB2\n",
                module: "second",
                timestamp: "ts4".to_string(),
            },
        ];
        let entries = history(&versions);
        let blamed = blame_file(&entries, "A\nB2\n");
        assert_eq!(blamed[0].gen, 1);
        assert_eq!(blamed[0].module, "first");
        assert_eq!(blamed[0].timestamp, "ts1");
        assert_eq!(blamed[0].text, "A");
        assert_eq!(blamed[1].gen, 4);
        assert_eq!(blamed[1].module, "second");
        assert_eq!(blamed[1].timestamp, "ts4");
        assert_eq!(blamed[1].text, "B2");
    }

    #[test]
    fn blame_line_at_returns_single_line() {
        let entries = history(&[ver(1, "A\nB\nC\n"), ver(2, "A\nB2\nC\n")]);
        assert_eq!(
            blame_line_at(&entries, "A\nB2\nC\n", 2).map(|b| b.gen),
            Some(2)
        );
        assert_eq!(
            blame_line_at(&entries, "A\nB2\nC\n", 1).map(|b| b.gen),
            Some(1)
        );
    }

    #[test]
    fn blame_line_at_out_of_range_is_none() {
        let entries = history(&[ver(1, "A\n")]);
        assert!(blame_line_at(&entries, "A\n", 9).is_none());
    }

    #[test]
    fn apply_creation_patch_tags_every_inserted_line() {
        let patch = file_patch(None, "x\ny\n");
        assert_eq!(apply_patch_to_blame(&[], &patch, 7), vec![7, 7]);
    }

    #[test]
    fn empty_patch_is_no_op() {
        assert_eq!(apply_patch_to_blame(&[1, 2, 3], "", 9), vec![1, 2, 3]);
    }

    #[test]
    fn first_prompt_diff_line_prefers_addition() {
        let diff = "@@ -1,2 +1,2 @@\n old\n-was this\n+now that\n";
        assert_eq!(first_prompt_diff_line(diff), "now that");
    }

    #[test]
    fn first_prompt_diff_line_falls_back_to_removal() {
        assert_eq!(
            first_prompt_diff_line("@@ -1,1 +0,0 @@\n-gone now\n"),
            "gone now"
        );
    }

    #[test]
    fn first_prompt_diff_line_empty() {
        assert_eq!(first_prompt_diff_line(""), "");
    }
}
