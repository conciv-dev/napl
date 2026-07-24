//! Conversions between NAPL's internal position model and LSP types, plus small
//! text helpers. NAPL positions are already zero-based UTF-16, matching LSP.

use std::path::Path;

use tower_lsp_server::ls_types::{Position, Range, Uri};

use napl_core::scanner::{Position as ScanPosition, Span as ScanSpan};

/// The number of UTF-16 code units in `s`, matching a JavaScript `string.length`.
#[must_use]
pub fn utf16_len(s: &str) -> usize {
    s.encode_utf16().count()
}

/// Split document text into lines, dropping a trailing `\r` per line.
#[must_use]
pub fn split_lines(text: &str) -> Vec<&str> {
    text.split('\n')
        .map(|seg| seg.strip_suffix('\r').unwrap_or(seg))
        .collect()
}

/// Build an LSP [`Position`] from zero-based `line` and UTF-16 `character`.
#[must_use]
pub fn position(line: usize, character: usize) -> Position {
    Position {
        line: line as u32,
        character: character as u32,
    }
}

/// Build an LSP [`Range`] from zero-based line/character bounds.
#[must_use]
pub fn range(start_line: usize, start_char: usize, end_line: usize, end_char: usize) -> Range {
    Range {
        start: position(start_line, start_char),
        end: position(end_line, end_char),
    }
}

/// Convert a scanner [`ScanPosition`] to an LSP [`Position`].
#[must_use]
pub fn scan_position(p: ScanPosition) -> Position {
    position(p.line, p.character)
}

/// Convert a scanner [`ScanSpan`] to an LSP [`Range`].
#[must_use]
pub fn scan_span(span: ScanSpan) -> Range {
    Range {
        start: scan_position(span.start),
        end: scan_position(span.end),
    }
}

/// The `file://` URI for a filesystem path, or `None` when it cannot be encoded.
#[must_use]
pub fn uri_for_path(path: &Path) -> Option<Uri> {
    Uri::from_file_path(path)
}
