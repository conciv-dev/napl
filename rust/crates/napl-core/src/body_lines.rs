//! Frontmatter/body line-offset math: mapping absolute document lines to
//! body-relative 1-based lines and back. Identical to the TS `body-lines`.

fn split_lines(text: &str) -> Vec<String> {
    text.split('\n')
        .map(|seg| seg.strip_suffix('\r').unwrap_or(seg).to_string())
        .collect()
}

/// The body of a prompt file together with the absolute document line index at
/// which it starts (0 when there is no frontmatter).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromptBody {
    pub body_start_line: usize,
    pub lines: Vec<String>,
}

/// Port of `promptBodyLines`: locate the body after a `---` delimited
/// frontmatter block, or treat the whole document as the body.
#[must_use]
pub fn prompt_body_lines(raw: &str) -> PromptBody {
    let lines = split_lines(raw);
    let has_frontmatter = lines.first().is_some_and(|l| l.trim_end() == "---");
    if has_frontmatter {
        for i in 1..lines.len() {
            if lines[i].trim_end() == "---" {
                let body_start_line = i + 1;
                let body = lines[body_start_line..].to_vec();
                return PromptBody {
                    body_start_line,
                    lines: body,
                };
            }
        }
    }
    PromptBody {
        body_start_line: 0,
        lines,
    }
}

/// Map an absolute document line (0-based, matching the TS caller convention)
/// to a 1-based body line, or `None` when it falls outside the body.
#[must_use]
pub fn body_line_for_doc_line(body: &PromptBody, doc_line: i64) -> Option<usize> {
    let body_line = doc_line - i64::try_from(body.body_start_line).ok()? + 1;
    if body_line < 1 || body_line > i64::try_from(body.lines.len()).ok()? {
        return None;
    }
    usize::try_from(body_line).ok()
}

/// Number lines 1-based for the model prompt, mirroring `numberLines`.
#[must_use]
pub fn number_lines(lines: &[String]) -> String {
    lines
        .iter()
        .enumerate()
        .map(|(index, line)| format!("{}: {line}", index + 1))
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw() -> &'static str {
        "---\nmodule: greeting\n---\nFirst body line.\nSecond body line."
    }

    #[test]
    fn locates_body_start_after_frontmatter() {
        let body = prompt_body_lines(raw());
        assert_eq!(body.body_start_line, 3);
        assert_eq!(body.lines[0], "First body line.");
    }

    #[test]
    fn maps_doc_lines_to_body_lines() {
        let body = prompt_body_lines(raw());
        assert_eq!(body_line_for_doc_line(&body, 3), Some(1));
        assert_eq!(body_line_for_doc_line(&body, 4), Some(2));
        assert_eq!(body_line_for_doc_line(&body, 2), None);
        assert_eq!(body_line_for_doc_line(&body, 99), None);
    }

    #[test]
    fn numbers_lines_1_based() {
        assert_eq!(
            number_lines(&["a".to_string(), "b".to_string()]),
            "1: a\n2: b"
        );
    }

    #[test]
    fn no_frontmatter_treats_all_as_body() {
        let body = prompt_body_lines("just a body\nsecond line");
        assert_eq!(body.body_start_line, 0);
        assert_eq!(body.lines.len(), 2);
        assert_eq!(body_line_for_doc_line(&body, 0), Some(1));
        assert_eq!(body_line_for_doc_line(&body, 1), Some(2));
    }

    #[test]
    fn empty_body_after_frontmatter() {
        let body = prompt_body_lines("---\nmodule: m\n---\n");
        assert_eq!(body.body_start_line, 3);
        assert_eq!(body.lines, vec![String::new()]);
    }
}
