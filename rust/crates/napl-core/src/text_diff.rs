//! LCS-based unified diff and hunk parsing.
//!
//! Output is byte-identical to the TypeScript `unifiedDiff`/`parseHunks`:
//! same headers, `context = 3`, and the same edge behaviors for empty
//! inputs and trailing-newline handling.

/// A single line inside a hunk, tagged by its unified-diff marker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HunkLine {
    pub kind: HunkKind,
    pub text: String,
}

/// The three unified-diff line markers: context, deletion, insertion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HunkKind {
    Context,
    Del,
    Ins,
}

/// A parsed unified-diff hunk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hunk {
    pub old_start: usize,
    pub old_count: usize,
    pub new_start: usize,
    pub new_count: usize,
    pub lines: Vec<HunkLine>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OpType {
    Equal,
    Del,
    Ins,
}

#[derive(Debug, Clone)]
struct DiffOp {
    op_type: OpType,
    old_line: usize,
    new_line: usize,
    text: String,
}

/// Split on `/\r?\n/`: `\n` is the separator, an immediately preceding `\r`
/// is consumed with it, a lone `\r` stays in the content.
fn split_lines(text: &str) -> Vec<String> {
    text.split('\n')
        .map(|seg| seg.strip_suffix('\r').unwrap_or(seg).to_string())
        .collect()
}

/// Port of the TS `toLines`: empty input yields no lines; otherwise a single
/// trailing `\r?\n` is dropped before splitting.
#[must_use]
pub fn to_lines(text: &str) -> Vec<String> {
    if text.is_empty() {
        return Vec::new();
    }
    let trimmed = if let Some(without_nl) = text.strip_suffix('\n') {
        without_nl.strip_suffix('\r').unwrap_or(without_nl)
    } else {
        text
    };
    split_lines(trimmed)
}

fn lcs_ops(a: &[String], b: &[String]) -> Vec<DiffOp> {
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
                op_type: OpType::Equal,
                old_line: i + 1,
                new_line: j + 1,
                text: a[i].clone(),
            });
            i += 1;
            j += 1;
        } else if dp[i + 1][j] >= dp[i][j + 1] {
            ops.push(DiffOp {
                op_type: OpType::Del,
                old_line: i + 1,
                new_line: j + 1,
                text: a[i].clone(),
            });
            i += 1;
        } else {
            ops.push(DiffOp {
                op_type: OpType::Ins,
                old_line: i + 1,
                new_line: j + 1,
                text: b[j].clone(),
            });
            j += 1;
        }
    }
    while i < n {
        ops.push(DiffOp {
            op_type: OpType::Del,
            old_line: i + 1,
            new_line: j + 1,
            text: a[i].clone(),
        });
        i += 1;
    }
    while j < m {
        ops.push(DiffOp {
            op_type: OpType::Ins,
            old_line: i + 1,
            new_line: j + 1,
            text: b[j].clone(),
        });
        j += 1;
    }
    ops
}

fn format_unified(ops: &[DiffOp], context: usize) -> String {
    let mut include = vec![false; ops.len()];
    for (idx, op) in ops.iter().enumerate() {
        if op.op_type == OpType::Equal {
            continue;
        }
        let from = idx.saturating_sub(context);
        let to = (idx + context).min(ops.len().saturating_sub(1));
        for slot in include.iter_mut().take(to + 1).skip(from) {
            *slot = true;
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
        let old_in_hunk: Vec<&&DiffOp> =
            hunk.iter().filter(|op| op.op_type != OpType::Ins).collect();
        let new_in_hunk: Vec<&&DiffOp> =
            hunk.iter().filter(|op| op.op_type != OpType::Del).collect();
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
            let sign = match op.op_type {
                OpType::Equal => ' ',
                OpType::Del => '-',
                OpType::Ins => '+',
            };
            lines.push(format!("{sign}{}", op.text));
        }
    }
    lines.join("\n")
}

/// Produce a unified diff between `before` and `after` with `context` lines
/// (default 3 via [`unified_diff`]).
#[must_use]
pub fn unified_diff_with_context(before: &str, after: &str, context: usize) -> String {
    format_unified(&lcs_ops(&to_lines(before), &to_lines(after)), context)
}

/// Produce a unified diff between `before` and `after` with 3 context lines.
#[must_use]
pub fn unified_diff(before: &str, after: &str) -> String {
    unified_diff_with_context(before, after, 3)
}

fn parse_header(raw: &str) -> Option<(usize, usize, usize, usize)> {
    let rest = raw.strip_prefix("@@ -")?;
    let (old_start, rest) = take_number(rest)?;
    let rest = rest.strip_prefix(',')?;
    let (old_count, rest) = take_number(rest)?;
    let rest = rest.strip_prefix(" +")?;
    let (new_start, rest) = take_number(rest)?;
    let rest = rest.strip_prefix(',')?;
    let (new_count, rest) = take_number(rest)?;
    rest.strip_prefix(" @@")?;
    Some((old_start, old_count, new_start, new_count))
}

fn take_number(s: &str) -> Option<(usize, &str)> {
    let end = s.find(|c: char| !c.is_ascii_digit()).unwrap_or(s.len());
    if end == 0 {
        return None;
    }
    let value = s[..end].parse().ok()?;
    Some((value, &s[end..]))
}

/// Parse the hunks out of a unified diff string, mirroring the TS `parseHunks`.
#[must_use]
pub fn parse_hunks(diff: &str) -> Vec<Hunk> {
    if diff.trim().is_empty() {
        return Vec::new();
    }
    let mut hunks: Vec<Hunk> = Vec::new();
    for raw in split_lines(diff) {
        if let Some((old_start, old_count, new_start, new_count)) = parse_header(&raw) {
            hunks.push(Hunk {
                old_start,
                old_count,
                new_start,
                new_count,
                lines: Vec::new(),
            });
            continue;
        }
        let Some(current) = hunks.last_mut() else {
            continue;
        };
        let marker = raw.chars().next();
        let kind = match marker {
            Some(' ') => HunkKind::Context,
            Some('-') => HunkKind::Del,
            Some('+') => HunkKind::Ins,
            _ => continue,
        };
        let text = raw[marker.map_or(0, char::len_utf8)..].to_string();
        current.lines.push(HunkLine { kind, text });
    }
    hunks
}

/// Apply parsed hunks to `before`, reconstructing the diff target. Used by the
/// roundtrip property tests.
#[must_use]
pub fn apply_hunks(before: &str, hunks: &[Hunk]) -> String {
    let old_lines = to_lines(before);
    let mut result: Vec<String> = Vec::new();
    let mut old_idx = 0usize;
    for hunk in hunks {
        let copy_until = hunk.old_start.saturating_sub(1);
        while old_idx < copy_until && old_idx < old_lines.len() {
            result.push(old_lines[old_idx].clone());
            old_idx += 1;
        }
        for line in &hunk.lines {
            match line.kind {
                HunkKind::Context => {
                    if old_idx < old_lines.len() {
                        result.push(old_lines[old_idx].clone());
                    } else {
                        result.push(line.text.clone());
                    }
                    old_idx += 1;
                }
                HunkKind::Del => {
                    old_idx += 1;
                }
                HunkKind::Ins => {
                    result.push(line.text.clone());
                }
            }
        }
    }
    while old_idx < old_lines.len() {
        result.push(old_lines[old_idx].clone());
        old_idx += 1;
    }
    result.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn to_lines_drops_single_trailing_newline_and_splits() {
        assert_eq!(to_lines("a\nb\n"), vec!["a", "b"]);
        assert_eq!(to_lines("a\nb"), vec!["a", "b"]);
    }

    #[test]
    fn to_lines_empty_is_empty() {
        assert!(to_lines("").is_empty());
    }

    #[test]
    fn to_lines_handles_crlf() {
        assert_eq!(to_lines("a\r\nb\r\n"), vec!["a", "b"]);
    }

    #[test]
    fn to_lines_keeps_lone_cr() {
        assert_eq!(to_lines("a\rb\n"), vec!["a\rb"]);
    }

    #[test]
    fn diff_empty_to_content_all_insert() {
        assert_eq!(unified_diff("", "a\nb\n"), "@@ -0,0 +1,2 @@\n+a\n+b");
    }

    #[test]
    fn diff_modification_scoped_header() {
        let diff = unified_diff("a\nb\nc\n", "a\nB\nc\n");
        let header = diff.split('\n').next().unwrap();
        assert!(header.starts_with("@@ -") && header.ends_with(" @@"));
        assert!(diff.contains("-b"));
        assert!(diff.contains("+B"));
    }

    #[test]
    fn diff_content_to_empty() {
        assert_eq!(unified_diff("a\nb\n", ""), "@@ -1,2 +0,0 @@\n-a\n-b");
    }

    #[test]
    fn parse_hunks_parses_headers_and_kinds() {
        let diff = "@@ -1,3 +1,3 @@\n a\n-b\n+B\n c\n";
        let hunks = parse_hunks(diff);
        assert_eq!(hunks.len(), 1);
        assert_eq!(hunks[0].old_start, 1);
        assert_eq!(hunks[0].old_count, 3);
        assert_eq!(hunks[0].new_start, 1);
        assert_eq!(hunks[0].new_count, 3);
        assert_eq!(
            hunks[0].lines.iter().map(|l| l.kind).collect::<Vec<_>>(),
            vec![
                HunkKind::Context,
                HunkKind::Del,
                HunkKind::Ins,
                HunkKind::Context
            ]
        );
    }

    #[test]
    fn parse_hunks_empty_diff() {
        assert!(parse_hunks("").is_empty());
    }

    #[test]
    fn roundtrip_diff_then_apply_reproduces_target() {
        let cases = [
            ("", "a\nb\nc\n"),
            ("a\nb\nc\n", ""),
            ("a\nb\nc\nd\ne\n", "a\nB\nc\nd\nE\nf\n"),
            ("one\ntwo\nthree\n", "one\ntwo\nthree\nfour\n"),
            ("x\ny\nz\n", "z\ny\nx\n"),
            ("same\nsame\n", "same\nsame\n"),
            ("a\nb\nc\nd\ne\nf\ng\n", "a\nc\ne\ng\n"),
        ];
        for (before, after) in cases {
            let diff = unified_diff(before, after);
            let hunks = parse_hunks(&diff);
            let rebuilt = apply_hunks(before, &hunks);
            let expected = to_lines(after).join("\n");
            assert_eq!(
                rebuilt, expected,
                "roundtrip failed for {before:?} -> {after:?}"
            );
        }
    }

    #[test]
    fn identical_inputs_produce_empty_diff() {
        assert_eq!(unified_diff("a\nb\nc\n", "a\nb\nc\n"), "");
    }
}
