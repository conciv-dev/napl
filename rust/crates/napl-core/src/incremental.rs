//! Prompt-body diffing with changed-line tracking, and the incremental
//! unlock-list computation. Ported from `incremental.ts`.

use crate::schemas::AttributionEntry;

/// A body-line diff: the unified text plus the changed old/new line numbers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BodyLineDiff {
    /// The unified diff text.
    pub unified: String,
    /// 1-based old-body line numbers that were deleted.
    pub changed_old_lines: Vec<usize>,
    /// 1-based new-body line numbers that were inserted.
    pub changed_new_lines: Vec<usize>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum OpType {
    Equal,
    Del,
    Ins,
}

struct DiffOp {
    kind: OpType,
    old_line: usize,
    new_line: usize,
    text: String,
}

fn split_lines(text: &str) -> Vec<&str> {
    text.split('\n')
        .map(|s| s.strip_suffix('\r').unwrap_or(s))
        .collect()
}

fn lcs_ops(a: &[&str], b: &[&str]) -> Vec<DiffOp> {
    let n = a.len();
    let m = b.len();
    let mut dp = vec![vec![0usize; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            dp[i][j] = if a[i] == b[j] {
                dp[i + 1][j + 1] + 1
            } else {
                dp[i + 1][j].max(dp[i][j + 1])
            };
        }
    }
    let mut ops = Vec::new();
    let mut i = 0;
    let mut j = 0;
    while i < n && j < m {
        if a[i] == b[j] {
            ops.push(DiffOp {
                kind: OpType::Equal,
                old_line: i + 1,
                new_line: j + 1,
                text: a[i].to_string(),
            });
            i += 1;
            j += 1;
        } else if dp[i + 1][j] >= dp[i][j + 1] {
            ops.push(DiffOp {
                kind: OpType::Del,
                old_line: i + 1,
                new_line: j + 1,
                text: a[i].to_string(),
            });
            i += 1;
        } else {
            ops.push(DiffOp {
                kind: OpType::Ins,
                old_line: i + 1,
                new_line: j + 1,
                text: b[j].to_string(),
            });
            j += 1;
        }
    }
    while i < n {
        ops.push(DiffOp {
            kind: OpType::Del,
            old_line: i + 1,
            new_line: j + 1,
            text: a[i].to_string(),
        });
        i += 1;
    }
    while j < m {
        ops.push(DiffOp {
            kind: OpType::Ins,
            old_line: i + 1,
            new_line: j + 1,
            text: b[j].to_string(),
        });
        j += 1;
    }
    ops
}

fn format_unified(ops: &[DiffOp], context: usize) -> String {
    let mut include = vec![false; ops.len()];
    for (idx, op) in ops.iter().enumerate() {
        if op.kind == OpType::Equal {
            continue;
        }
        let from = idx.saturating_sub(context);
        let to = (idx + context).min(ops.len().saturating_sub(1));
        for item in include.iter_mut().take(to + 1).skip(from) {
            *item = true;
        }
    }

    let mut hunks: Vec<Vec<&DiffOp>> = Vec::new();
    let mut current: Vec<&DiffOp> = Vec::new();
    for (idx, op) in ops.iter().enumerate() {
        if include[idx] {
            current.push(op);
        } else if !current.is_empty() {
            hunks.push(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        hunks.push(current);
    }

    let mut lines: Vec<String> = Vec::new();
    for hunk in hunks {
        let old_in_hunk: Vec<&&DiffOp> = hunk.iter().filter(|op| op.kind != OpType::Ins).collect();
        let new_in_hunk: Vec<&&DiffOp> = hunk.iter().filter(|op| op.kind != OpType::Del).collect();
        let old_start = old_in_hunk.first().map_or(0, |op| op.old_line);
        let new_start = new_in_hunk.first().map_or(0, |op| op.new_line);
        lines.push(format!(
            "@@ -{},{} +{},{} @@",
            old_start,
            old_in_hunk.len(),
            new_start,
            new_in_hunk.len()
        ));
        for op in hunk {
            let sign = match op.kind {
                OpType::Equal => ' ',
                OpType::Del => '-',
                OpType::Ins => '+',
            };
            lines.push(format!("{sign}{}", op.text));
        }
    }
    lines.join("\n")
}

/// Diff two prompt bodies, mirroring `diffBodyLines` (context = 3).
#[must_use]
pub fn diff_body_lines(old_body: &str, new_body: &str) -> BodyLineDiff {
    let a = split_lines(old_body);
    let b = split_lines(new_body);
    let ops = lcs_ops(&a, &b);
    let changed_old_lines = ops
        .iter()
        .filter(|op| op.kind == OpType::Del)
        .map(|op| op.old_line)
        .collect();
    let changed_new_lines = ops
        .iter()
        .filter(|op| op.kind == OpType::Ins)
        .map(|op| op.new_line)
        .collect();
    BodyLineDiff {
        unified: format_unified(&ops, 3),
        changed_old_lines,
        changed_new_lines,
    }
}

/// Attribution entries whose prompt range intersects the changed old lines,
/// mirroring `selectIntersectingEntries`.
#[must_use]
pub fn select_intersecting_entries(
    entries: &[AttributionEntry],
    changed_old_lines: &[usize],
) -> Vec<AttributionEntry> {
    entries
        .iter()
        .filter(|entry| {
            (entry.prompt_lines.start..=entry.prompt_lines.end)
                .any(|line| changed_old_lines.contains(&(line as usize)))
        })
        .cloned()
        .collect()
}

/// The sorted unlock list: owned files plus the target-relative paths of the
/// intersecting regions, mirroring `incrementalUnlockList`.
#[must_use]
pub fn incremental_unlock_list(
    owned_files: &[String],
    intersecting_entries: &[AttributionEntry],
    target_rel_to_root: &str,
) -> Vec<String> {
    let mut set: Vec<String> = owned_files.to_vec();
    for entry in intersecting_entries {
        set.push(format!("{target_rel_to_root}/{}", entry.file));
    }
    set.sort();
    set.dedup();
    set
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diff_tracks_changed_lines_and_unified() {
        let diff = diff_body_lines(
            "Greet a person by name.\n",
            "Greet a person by name, loudly.\n",
        );
        assert!(diff.unified.contains("-Greet a person by name."));
        assert!(diff.unified.contains("+Greet a person by name, loudly."));
        assert_eq!(diff.changed_old_lines, vec![1]);
        assert_eq!(diff.changed_new_lines, vec![1]);
    }

    #[test]
    fn unlock_list_is_sorted_and_deduped() {
        let list = incremental_unlock_list(
            &[".napl/src/typescript/greet.ts".to_string()],
            &[],
            ".napl/src/typescript",
        );
        assert_eq!(list, vec![".napl/src/typescript/greet.ts".to_string()]);
    }
}
