//! Span scanner for prompt files.
//!
//! All positions are **zero-based `line` / `character`, where `character` is a
//! count of UTF-16 code units** (not bytes and not Unicode scalar values). This
//! matches the LSP position model and the TypeScript `scanner.ts` exactly, so
//! that a multibyte or astral character (e.g. an emoji) before a token shifts
//! the reported column by that character's UTF-16 length (1 or 2).

/// A zero-based position; `character` is a UTF-16 code-unit offset within the line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Position {
    pub line: usize,
    pub character: usize,
}

/// A half-open-in-spirit inclusive span between two [`Position`]s.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub start: Position,
    pub end: Position,
}

/// A region (frontmatter or body) that may or may not be present.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegionSpan {
    pub present: bool,
    pub span: Option<Span>,
}

/// A frontmatter key token and the span of the key itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrontmatterKeyToken {
    pub key: String,
    pub key_span: Span,
}

/// The `module:` value token.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleValueToken {
    pub value: String,
    pub span: Span,
}

/// Which frontmatter key a dependency token came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DepSource {
    Deps,
    Extends,
}

/// A dependency entry (`deps`/`extends`) token.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DepToken {
    pub value: String,
    pub span: Span,
    pub source: DepSource,
}

/// An `@module/ref` token found in the body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefToken {
    pub module: String,
    pub span: Span,
}

/// The full result of scanning a prompt document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanResult {
    pub frontmatter: RegionSpan,
    pub body: RegionSpan,
    pub keys: Vec<FrontmatterKeyToken>,
    pub module_value: Option<ModuleValueToken>,
    pub deps: Vec<DepToken>,
    pub refs: Vec<RefToken>,
}

/// A resolved navigation target at a position.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    ModuleValue {
        module: String,
        span: Span,
    },
    Dep {
        module: String,
        span: Span,
        source: DepSource,
    },
    Ref {
        module: String,
        span: Span,
    },
}

fn split_lines(text: &str) -> Vec<String> {
    text.split('\n')
        .map(|seg| seg.strip_suffix('\r').unwrap_or(seg).to_string())
        .collect()
}

fn utf16_len(s: &str) -> usize {
    s.encode_utf16().count()
}

fn utf16_offset(s: &str, byte_idx: usize) -> usize {
    s[..byte_idx].encode_utf16().count()
}

fn pos(line: usize, character: usize) -> Position {
    Position { line, character }
}

fn span(start_line: usize, start_char: usize, end_line: usize, end_char: usize) -> Span {
    Span {
        start: pos(start_line, start_char),
        end: pos(end_line, end_char),
    }
}

/// Strip a single pair of matching ASCII quotes; returns the inner value and the
/// UTF-16 offset introduced (0 or 1).
fn strip_quotes(raw: &str) -> (&str, usize) {
    let first = raw.chars().next();
    let last = raw.chars().last();
    if let (Some(f), Some(l)) = (first, last) {
        if (f == '"' || f == '\'') && raw.chars().count() >= 2 && l == f {
            return (&raw[f.len_utf8()..raw.len() - l.len_utf8()], 1);
        }
    }
    (raw, 0)
}

fn is_item_char(c: char) -> bool {
    c != ',' && c != '[' && c != ']'
}

fn scan_inline_list(
    inner: &str,
    base_line: usize,
    base_char: usize,
    source: DepSource,
) -> Vec<DepToken> {
    let mut tokens = Vec::new();
    let chars: Vec<(usize, char)> = inner.char_indices().collect();
    let mut idx = 0;
    while idx < chars.len() {
        let (start_byte, c) = chars[idx];
        if is_item_char(c) && !c.is_whitespace() {
            let mut end_idx = idx + 1;
            while end_idx < chars.len() && is_item_char(chars[end_idx].1) {
                end_idx += 1;
            }
            let end_byte = if end_idx < chars.len() {
                chars[end_idx].0
            } else {
                inner.len()
            };
            let matched = &inner[start_byte..end_byte];
            let raw_item = matched.trim_end();
            if !raw_item.is_empty() {
                let start_in_inner = utf16_offset(inner, start_byte);
                let (value, offset) = strip_quotes(raw_item);
                let start_char = base_char + start_in_inner + offset;
                tokens.push(DepToken {
                    value: value.to_string(),
                    source,
                    span: span(
                        base_line,
                        start_char,
                        base_line,
                        start_char + utf16_len(value),
                    ),
                });
            }
            idx = end_idx;
        } else {
            idx += 1;
        }
    }
    tokens
}

fn scan_value_token(
    raw_after_colon: &str,
    line: usize,
    key_end_char: usize,
) -> Option<(String, Span)> {
    let leading = utf16_len(raw_after_colon) - utf16_len(raw_after_colon.trim_start());
    let trimmed = raw_after_colon.trim();
    if trimmed.is_empty() {
        return None;
    }
    let start_char = key_end_char + leading;
    let (value, offset) = strip_quotes(trimmed);
    let val_start = start_char + offset;
    Some((
        value.to_string(),
        span(line, val_start, line, val_start + utf16_len(value)),
    ))
}

/// Match `/^([A-Za-z0-9_-]+):(.*)$/`, returning `(key, after_colon)`.
fn match_key(raw: &str) -> Option<(&str, &str)> {
    let mut colon_byte = None;
    for (b, c) in raw.char_indices() {
        if c == ':' {
            colon_byte = Some(b);
            break;
        }
        if !(c.is_ascii_alphanumeric() || c == '_' || c == '-') {
            return None;
        }
    }
    let colon = colon_byte?;
    if colon == 0 {
        return None;
    }
    Some((&raw[..colon], &raw[colon + 1..]))
}

/// Match `/^(\s*)-\s+(.*)$/`, returning `(indent_utf16, value_start_char_utf16, rest)`.
fn match_list_item(raw: &str) -> Option<(usize, usize, &str)> {
    let ws_bytes: usize = raw
        .char_indices()
        .take_while(|(_, c)| c.is_whitespace())
        .map(|(_, c)| c.len_utf8())
        .sum();
    let indent = utf16_len(&raw[..ws_bytes]);
    let after = &raw[ws_bytes..];
    let mut chars = after.char_indices();
    match chars.next() {
        Some((_, '-')) => {}
        _ => return None,
    }
    let mut ws_count = 0usize;
    let mut rest_byte = None;
    for (b, c) in after.char_indices().skip(1) {
        if c.is_whitespace() {
            ws_count += 1;
        } else {
            rest_byte = Some(b);
            break;
        }
    }
    if ws_count == 0 {
        // `-` must be followed by at least one whitespace (`\s+`).
        // If the rest is empty (dash at end) there is no `\s+` either.
        return None;
    }
    let dash_and_space = 1 + ws_count;
    let value_start_char = indent + dash_and_space;
    let rest = match rest_byte {
        Some(b) => &after[b..],
        None => "",
    };
    Some((indent, value_start_char, rest))
}

/// Scan a prompt document into structured tokens with UTF-16 spans.
#[must_use]
pub fn scan_document(text: &str) -> ScanResult {
    let lines = split_lines(text);
    let mut keys: Vec<FrontmatterKeyToken> = Vec::new();
    let mut deps: Vec<DepToken> = Vec::new();
    let mut refs: Vec<RefToken> = Vec::new();
    let mut module_value: Option<ModuleValueToken> = None;

    let mut frontmatter = RegionSpan {
        present: false,
        span: None,
    };
    let body: RegionSpan;

    let has_frontmatter = lines.first().is_some_and(|l| l.trim_end() == "---");
    let mut close_line: Option<usize> = None;
    if has_frontmatter {
        for (i, line) in lines.iter().enumerate().skip(1) {
            if line.trim_end() == "---" {
                close_line = Some(i);
                break;
            }
        }
    }

    if let (true, Some(close)) = (has_frontmatter, close_line) {
        let inner_start = 1usize;
        let inner_end = close - 1;
        frontmatter = RegionSpan {
            present: true,
            span: Some(if inner_end >= inner_start {
                span(inner_start, 0, inner_end, utf16_len(&lines[inner_end]))
            } else {
                span(inner_start, 0, inner_start, 0)
            }),
        };

        let mut current_list_key: Option<DepSource> = None;
        if inner_end >= inner_start {
            for (offset, raw) in lines[inner_start..=inner_end].iter().enumerate() {
                let i = inner_start + offset;
                if let (Some(source), Some((_, value_start_char, rest))) =
                    (current_list_key, match_list_item(raw))
                {
                    let raw_item = rest.trim();
                    if !raw_item.is_empty() {
                        let (value, offset) = strip_quotes(raw_item);
                        let start_char = value_start_char + offset;
                        deps.push(DepToken {
                            value: value.to_string(),
                            source,
                            span: span(i, start_char, i, start_char + utf16_len(value)),
                        });
                    }
                    continue;
                }

                let Some((key, after_colon)) = match_key(raw) else {
                    continue;
                };
                current_list_key = None;
                let key_end_char = utf16_len(key) + 1;
                keys.push(FrontmatterKeyToken {
                    key: key.to_string(),
                    key_span: span(i, 0, i, utf16_len(key)),
                });

                if key == "module" {
                    if let Some((text, sp)) = scan_value_token(after_colon, i, key_end_char) {
                        module_value = Some(ModuleValueToken {
                            value: text,
                            span: sp,
                        });
                    }
                    continue;
                }

                if key == "deps" || key == "extends" {
                    let source = if key == "deps" {
                        DepSource::Deps
                    } else {
                        DepSource::Extends
                    };
                    let trimmed_after = after_colon.trim();
                    if trimmed_after.starts_with('[') {
                        let open_idx = after_colon.find('[').unwrap();
                        let close_idx = after_colon.rfind(']');
                        let inner = match close_idx {
                            Some(ci) if ci > open_idx => &after_colon[open_idx + 1..ci],
                            _ => &after_colon[open_idx + 1..],
                        };
                        let base_char = key_end_char + utf16_offset(after_colon, open_idx) + 1;
                        deps.extend(scan_inline_list(inner, i, base_char, source));
                    } else if trimmed_after.is_empty() {
                        current_list_key = Some(source);
                    } else if let Some((text, sp)) = scan_value_token(after_colon, i, key_end_char)
                    {
                        deps.push(DepToken {
                            value: text,
                            source,
                            span: sp,
                        });
                    }
                }
            }
        }

        let body_start_line = close + 1;
        if body_start_line < lines.len() {
            body = RegionSpan {
                present: true,
                span: Some(span(
                    body_start_line,
                    0,
                    lines.len() - 1,
                    utf16_len(&lines[lines.len() - 1]),
                )),
            };
        } else {
            body = RegionSpan {
                present: false,
                span: None,
            };
        }
    } else {
        body = RegionSpan {
            present: !lines.is_empty(),
            span: if lines.is_empty() {
                None
            } else {
                Some(span(
                    0,
                    0,
                    lines.len() - 1,
                    utf16_len(&lines[lines.len() - 1]),
                ))
            },
        };
    }

    let body_start = match close_line {
        Some(close) if has_frontmatter => close + 1,
        _ => 0,
    };
    for (i, line) in lines.iter().enumerate().skip(body_start) {
        scan_refs_in_line(line, i, &mut refs);
    }

    ScanResult {
        frontmatter,
        body,
        keys,
        module_value,
        deps,
        refs,
    }
}

fn is_ref_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '/' || c == '-'
}

fn scan_refs_in_line(line: &str, line_no: usize, refs: &mut Vec<RefToken>) {
    let chars: Vec<(usize, char)> = line.char_indices().collect();
    let mut idx = 0;
    while idx < chars.len() {
        let (at_byte, c) = chars[idx];
        if c == '@' {
            let mut end_idx = idx + 1;
            while end_idx < chars.len() && is_ref_char(chars[end_idx].1) {
                end_idx += 1;
            }
            if end_idx > idx + 1 {
                let end_byte = if end_idx < chars.len() {
                    chars[end_idx].0
                } else {
                    line.len()
                };
                let full = &line[at_byte..end_byte];
                let start_char = utf16_offset(line, at_byte);
                refs.push(RefToken {
                    module: full[1..].to_string(),
                    span: span(line_no, start_char, line_no, start_char + utf16_len(full)),
                });
                idx = end_idx;
                continue;
            }
        }
        idx += 1;
    }
}

fn within_span(target: Span, position: Position) -> bool {
    let Span { start, end } = target;
    if position.line < start.line || position.line > end.line {
        return false;
    }
    if position.line == start.line && position.character < start.character {
        return false;
    }
    if position.line == end.line && position.character > end.character {
        return false;
    }
    true
}

/// Resolve the navigation target at `position`, mirroring the TS precedence:
/// module value, then deps, then refs.
#[must_use]
pub fn find_target_at_position(scan: &ScanResult, position: Position) -> Option<Target> {
    if let Some(mv) = &scan.module_value {
        if within_span(mv.span, position) {
            return Some(Target::ModuleValue {
                module: mv.value.clone(),
                span: mv.span,
            });
        }
    }
    for dep in &scan.deps {
        if within_span(dep.span, position) {
            return Some(Target::Dep {
                module: dep.value.clone(),
                span: dep.span,
                source: dep.source,
            });
        }
    }
    for r in &scan.refs {
        if within_span(r.span, position) {
            return Some(Target::Ref {
                module: r.module.clone(),
                span: r.span,
            });
        }
    }
    None
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
