//! Prompt-file diagnostics: frontmatter validity, DRIFT / prompt-stale status,
//! and machine-layer (.mapl) squiggles. Generated files carry no diagnostics.

use std::path::Path;

use tower_lsp_server::ls_types::{Diagnostic, DiagnosticSeverity};

use napl_core::body_lines::prompt_body_lines;
use napl_core::reverse::parse_generated_path;
use napl_core::scanner::scan_document;

use crate::classify::{classify_prompt, PromptStatus};
use crate::context::rel_to;
use crate::convert::{range, split_lines};
use crate::ml::ml_diagnostics;
use crate::state::{find_workspace_root, load_ml, read_map};

/// Compute the diagnostics to publish for a document, or an empty vector when it
/// is outside a workspace or is a generated source file.
#[must_use]
pub fn compute(doc_path: &Path, doc_text: &str) -> Vec<Diagnostic> {
    let Some(root) = find_workspace_root(doc_path) else {
        return Vec::new();
    };
    let rel_path = rel_to(&root, doc_path);
    if parse_generated_path(&rel_path).is_some() {
        return Vec::new();
    }
    compute_prompt_diagnostics(&root, &rel_path, doc_text)
}

fn compute_prompt_diagnostics(root: &Path, rel_path: &str, text: &str) -> Vec<Diagnostic> {
    let mut diagnostics: Vec<Diagnostic> = Vec::new();
    let scan = scan_document(text);

    if scan.frontmatter.present {
        if let Some(span) = scan.frontmatter.span {
            let doc_lines = split_lines(text);
            let inner_start = span.start.line;
            let inner_end = span.end.line.min(doc_lines.len().saturating_sub(1));
            let inner = doc_lines
                .get(inner_start..=inner_end)
                .unwrap_or(&[])
                .join("\n");
            if let Err(error) = serde_yaml::from_str::<serde_yaml::Value>(&inner) {
                let (line, character) = error
                    .location()
                    .map_or((inner_start, 0), |loc| {
                        (inner_start + loc.line().saturating_sub(1), loc.column().saturating_sub(1))
                    });
                diagnostics.push(error_diagnostic(
                    line,
                    character,
                    character + 1,
                    format!("YAML frontmatter error: {error}"),
                ));
                return diagnostics;
            }
        }
    } else {
        diagnostics.push(error_diagnostic(
            0,
            0,
            1,
            "missing YAML frontmatter: a prompt file must start with a --- delimited block"
                .to_string(),
        ));
        return diagnostics;
    }

    let map = read_map(root);
    let status = match classify_prompt(root, rel_path, text, &map) {
        Ok(status) => status,
        Err(message) => {
            diagnostics.push(error_diagnostic(0, 0, 3, message));
            return diagnostics;
        }
    };

    match status {
        PromptStatus::Drift { target, file } => {
            let message = format!(
                "DRIFT: generated file {file} was edited — it no longer matches the prompt. \
Resolve: (1) napl reconcile {module} to fold the edit into your prompt (coming soon); \
(2) napl gen {target} --module {module} --force to discard the edit; \
(3) edit the prompt to describe the change, then napl gen {target}.",
                module = module_of(text)
            );
            diagnostics.push(Diagnostic {
                range: range(0, 0, 0, 3),
                severity: Some(DiagnosticSeverity::ERROR),
                message,
                source: Some("napl".to_string()),
                ..Diagnostic::default()
            });
        }
        PromptStatus::PromptStale { detail } => {
            diagnostics.push(Diagnostic {
                range: range(0, 0, 0, 3),
                severity: Some(DiagnosticSeverity::INFORMATION),
                message: format!("prompt changed since last gen ({detail}) — run napl gen"),
                source: Some("napl".to_string()),
                ..Diagnostic::default()
            });
        }
        PromptStatus::Clean | PromptStatus::Unattributed => {}
    }

    append_ml_diagnostics(root, text, &mut diagnostics);
    diagnostics
}

fn append_ml_diagnostics(root: &Path, text: &str, diagnostics: &mut Vec<Diagnostic>) {
    let module = module_of(text);
    if module.is_empty() {
        return;
    }
    let Some(ml) = load_ml(root, &module) else {
        return;
    };
    let body = prompt_body_lines(text);
    let doc_lines = split_lines(text);
    diagnostics.extend(ml_diagnostics(&ml, body.body_start_line, &doc_lines));
}

fn module_of(text: &str) -> String {
    napl_core::schemas::parse_frontmatter(text)
        .map(|parsed| parsed.frontmatter.module)
        .unwrap_or_default()
}

fn error_diagnostic(line: usize, start_char: usize, end_char: usize, message: String) -> Diagnostic {
    Diagnostic {
        range: range(line, start_char, line, end_char),
        severity: Some(DiagnosticSeverity::ERROR),
        message,
        source: Some("napl".to_string()),
        ..Diagnostic::default()
    }
}
