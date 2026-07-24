//! Shared resolution helpers used by the hover, navigation, and diagnostics
//! layers: workspace-relative paths, prompt/generated context, prompt-span
//! locations, and the mechanical (blame) label.

use std::path::Path;

use tower_lsp_server::ls_types::Location;

use napl_core::blame::first_prompt_diff_line;
use napl_core::body_lines::{body_line_for_doc_line, prompt_body_lines};
use napl_core::reverse::{
    match_prompt_lines, parse_generated_path, AttributionSource, GeneratedPathInfo, ReverseMatch,
};
use napl_core::schemas::{parse_frontmatter, NaplMap};

use crate::convert::{range, split_lines, uri_for_path, utf16_len};
use crate::state::{load_attribution_sources, read_map, Mechanical};

/// A generated file resolved against its workspace.
pub struct GeneratedContext {
    pub rel_full: String,
    pub info: GeneratedPathInfo,
    pub map: NaplMap,
    pub sources: Vec<AttributionSource>,
}

/// The prompt line context at a document position.
pub struct BodyContext {
    pub module: String,
    pub body_line: u32,
}

/// The POSIX-style path of `path` relative to `root`.
#[must_use]
pub fn rel_to(root: &Path, path: &Path) -> String {
    let rel = path.strip_prefix(root).unwrap_or(path);
    rel.to_string_lossy()
        .replace(std::path::MAIN_SEPARATOR, "/")
}

/// Resolve a document path into generated-file context, or `None` when it is not
/// a generated source file under `.napl/src/`.
#[must_use]
pub fn resolve_generated_context(root: &Path, doc_path: &Path) -> Option<GeneratedContext> {
    let rel_full = rel_to(root, doc_path);
    let info = parse_generated_path(&rel_full)?;
    let map = read_map(root);
    let sources = load_attribution_sources(root, &map);
    Some(GeneratedContext {
        rel_full,
        info,
        map,
        sources,
    })
}

/// The prompt module and body-relative line at `line`, or `None` when the
/// frontmatter is invalid or the position is outside the body.
#[must_use]
pub fn resolve_body_context(text: &str, line: u32) -> Option<BodyContext> {
    let module = parse_frontmatter(text).ok()?.frontmatter.module;
    let body = prompt_body_lines(text);
    let body_line = body_line_for_doc_line(&body, i64::from(line))?;
    Some(BodyContext {
        module,
        body_line: body_line as u32,
    })
}

/// The prompt-file [`Location`] a reverse match points to, or `None` when the
/// prompt file is missing.
#[must_use]
pub fn prompt_location(root: &Path, m: &ReverseMatch) -> Option<Location> {
    let abs = root.join(&m.prompt_file);
    let text = std::fs::read_to_string(&abs).ok()?;
    let body = prompt_body_lines(&text);
    let (start_line, end_line) = match_prompt_lines(&body, m.prompt_lines);
    let doc_lines = split_lines(&text);
    let end_char = doc_lines.get(end_line).map_or(0, |line| utf16_len(line));
    Some(Location {
        uri: uri_for_path(&abs)?,
        range: range(start_line, 0, end_line, end_char),
    })
}

/// The "caused by gen …" label for a generated line, or `None` when the line has
/// no blame.
#[must_use]
pub fn mechanical_label(mechanical: &Mechanical, line1: usize) -> Option<String> {
    let blamed = mechanical.blamed.iter().find(|entry| entry.line == line1)?;
    let empty = String::new();
    let diff = mechanical.prompt_diff_by_gen.get(&blamed.gen).unwrap_or(&empty);
    let first = first_prompt_diff_line(diff);
    let date = blamed.timestamp.get(..10).unwrap_or(&blamed.timestamp);
    let edit = if first.is_empty() {
        "initial generation".to_string()
    } else {
        format!("prompt edit: {first}")
    };
    Some(format!("caused by gen #{} · {date} · {edit}", blamed.gen))
}

/// The Markdown fence language for a file extension.
#[must_use]
pub fn fence_lang(file_path: &str) -> &'static str {
    let ext = std::path::Path::new(file_path)
        .extension()
        .map(|ext| ext.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    match ext.as_str() {
        "ts" | "tsx" => "ts",
        "js" | "jsx" => "js",
        "css" => "css",
        "html" => "html",
        _ => "text",
    }
}

fn is_test_file(file_path: &str) -> bool {
    let lower = file_path.to_ascii_lowercase();
    if let Some(dot) = lower.rfind(".test.") {
        lower[dot + ".test.".len()..]
            .chars()
            .all(|c| c.is_ascii_alphabetic())
            && !lower[dot + ".test.".len()..].is_empty()
    } else {
        false
    }
}

/// Order files so implementation files precede test files (a stable partition).
#[must_use]
pub fn ordered_files<T, F: Fn(&T) -> &str>(files: Vec<T>, path_of: F) -> Vec<T> {
    let mut impls = Vec::new();
    let mut tests = Vec::new();
    for file in files {
        if is_test_file(path_of(&file)) {
            tests.push(file);
        } else {
            impls.push(file);
        }
    }
    impls.extend(tests);
    impls
}
