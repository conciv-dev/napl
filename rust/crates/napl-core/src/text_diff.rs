//! Stage1 adapter over the NAPL-generated `text_diff` crate.

pub use gen_text_diff::{
    apply_hunks, parse_hunks, to_lines, unified_diff, unified_diff_with_context, Hunk, HunkKind,
    HunkLine,
};

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
