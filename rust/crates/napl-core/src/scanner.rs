//! Stage1 adapter over the NAPL-generated `scanner` crate. The generated crate
//! names its scan entry point `scan`; the hand-written module named it
//! `scan_document`, bridged by the rename below.

pub use gen_scanner::{
    find_target_at_position, scan as scan_document, DepSource, DepToken, FrontmatterKeyToken,
    ModuleValueToken, Position, RefToken, RegionSpan, ScanResult, Span, Target,
};

#[cfg(test)]
fn pos(line: usize, character: usize) -> Position {
    Position { line, character }
}

#[cfg(test)]
fn span(start_line: usize, start_char: usize, end_line: usize, end_char: usize) -> Span {
    Span {
        start: pos(start_line, start_char),
        end: pos(end_line, end_char),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DOC: &str = "---\nmodule: auth/session\ndeps: [auth/tokens, auth/users]\ntargets: [typescript]\n---\n# Session\n\nManage sessions. See @auth/tokens and @auth/users for details.\nRefreshes via @auth/session.\n";

    #[test]
    fn locates_module_value_span_exactly() {
        let scan = scan_document(DOC);
        let mv = scan.module_value.as_ref().unwrap();
        assert_eq!(mv.value, "auth/session");
        assert_eq!(mv.span, span(1, 8, 1, 20));
    }

    #[test]
    fn locates_inline_deps_with_exact_spans() {
        let scan = scan_document(DOC);
        assert_eq!(
            scan.deps
                .iter()
                .map(|d| d.value.as_str())
                .collect::<Vec<_>>(),
            vec!["auth/tokens", "auth/users"]
        );
        assert!(scan.deps.iter().all(|d| d.source == DepSource::Deps));
        assert_eq!(scan.deps[0].span, span(2, 7, 2, 18));
        assert_eq!(scan.deps[1].span.start, pos(2, 20));
        assert_eq!(scan.deps[1].span.end, pos(2, 30));
    }

    #[test]
    fn finds_refs_in_body_excluding_frontmatter() {
        let scan = scan_document(DOC);
        assert_eq!(
            scan.refs
                .iter()
                .map(|r| r.module.as_str())
                .collect::<Vec<_>>(),
            vec!["auth/tokens", "auth/users", "auth/session"]
        );
        assert_eq!(scan.refs[0].span.start, pos(7, 21));
        assert_eq!(scan.refs[0].span.end, pos(7, 33));
    }

    #[test]
    fn reports_frontmatter_and_body_regions() {
        let scan = scan_document(DOC);
        assert!(scan.frontmatter.present);
        assert_eq!(scan.frontmatter.span.unwrap().start, pos(1, 0));
        assert_eq!(scan.frontmatter.span.unwrap().end.line, 3);
        assert!(scan.body.present);
        assert_eq!(scan.body.span.unwrap().start.line, 5);
    }

    #[test]
    fn handles_block_style_deps_lists() {
        let doc = "---\nmodule: a\ndeps:\n  - one\n  - \"two/x\"\nextends: base\n---\nbody\n";
        let scan = scan_document(doc);
        assert_eq!(
            scan.deps
                .iter()
                .map(|d| format!(
                    "{}:{}",
                    match d.source {
                        DepSource::Deps => "deps",
                        DepSource::Extends => "extends",
                    },
                    d.value
                ))
                .collect::<Vec<_>>(),
            vec!["deps:one", "deps:two/x", "extends:base"]
        );
        assert_eq!(scan.deps[0].span, span(3, 4, 3, 7));
        assert_eq!(scan.deps[1].span.start, pos(4, 5));
        assert_eq!(scan.deps[2].span.start, pos(5, 9));
    }

    #[test]
    fn handles_empty_inline_deps() {
        let scan = scan_document("---\nmodule: greeting\ndeps: []\n---\nbody\n");
        assert!(scan.deps.is_empty());
        assert_eq!(scan.module_value.unwrap().value, "greeting");
    }

    #[test]
    fn resolves_module_value_token() {
        let scan = scan_document(DOC);
        let target = find_target_at_position(&scan, pos(1, 10));
        assert_eq!(
            target,
            Some(Target::ModuleValue {
                module: "auth/session".to_string(),
                span: scan.module_value.unwrap().span,
            })
        );
    }

    #[test]
    fn resolves_dep_token() {
        let scan = scan_document(DOC);
        let target = find_target_at_position(&scan, pos(2, 10));
        assert!(matches!(target, Some(Target::Dep { ref module, .. }) if module == "auth/tokens"));
    }

    #[test]
    fn resolves_ref_token() {
        let scan = scan_document(DOC);
        let target = find_target_at_position(&scan, pos(7, 25));
        assert!(matches!(target, Some(Target::Ref { ref module, .. }) if module == "auth/tokens"));
    }

    #[test]
    fn returns_null_off_any_token() {
        let scan = scan_document(DOC);
        assert_eq!(find_target_at_position(&scan, pos(5, 0)), None);
    }

    #[test]
    fn multibyte_content_shifts_ref_column_by_utf16_units() {
        // A single astral emoji is 2 UTF-16 code units; two of them push the @ref
        // start to character 4, and "b" (1 unit) to character 1.
        let doc = "---\nmodule: m\n---\n\u{1F600}\u{1F600}@auth/x\n";
        let scan = scan_document(doc);
        assert_eq!(scan.refs.len(), 1);
        assert_eq!(scan.refs[0].module, "auth/x");
        assert_eq!(scan.refs[0].span.start, pos(3, 4));
        assert_eq!(scan.refs[0].span.end, pos(3, 4 + 7));
    }

    #[test]
    fn bmp_multibyte_counts_as_one_utf16_unit() {
        // U+00E9 (é) is one UTF-16 unit but two UTF-8 bytes.
        let doc = "---\nmodule: m\n---\n\u{00E9}@x/y\n";
        let scan = scan_document(doc);
        assert_eq!(scan.refs[0].span.start, pos(3, 1));
    }
}
