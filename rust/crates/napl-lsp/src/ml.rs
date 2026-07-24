//! Machine-layer (.mapl) diagnostics and hover rendering. Ambiguity surfaces as
//! an error, assumption and no-op as warnings, note as information — the same
//! mapping the TypeScript LSP used.

use tower_lsp_server::ls_types::{Diagnostic, DiagnosticSeverity};

use napl_core::schemas::{Ml, MlEntry, MlKind};

use crate::convert::{range, utf16_len};

/// The diagnostic severity for a machine-layer entry kind.
#[must_use]
pub fn ml_severity(kind: MlKind) -> DiagnosticSeverity {
    match kind {
        MlKind::Ambiguity => DiagnosticSeverity::ERROR,
        MlKind::Assumption | MlKind::NoOp => DiagnosticSeverity::WARNING,
        MlKind::Note => DiagnosticSeverity::INFORMATION,
    }
}

/// One diagnostic per machine-layer entry, positioned at its prompt span.
#[must_use]
pub fn ml_diagnostics(ml: &Ml, body_start_line: usize, doc_lines: &[&str]) -> Vec<Diagnostic> {
    ml.entries
        .iter()
        .map(|entry| {
            let start_line = body_start_line + entry.prompt_lines.start as usize - 1;
            let end_line = body_start_line + entry.prompt_lines.end as usize - 1;
            let end_char = doc_lines.get(end_line).map_or(200, |line| utf16_len(line));
            Diagnostic {
                range: range(start_line, 0, end_line, end_char),
                severity: Some(ml_severity(entry.kind)),
                message: entry.message.clone(),
                source: Some("napl-mapl".to_string()),
                ..Diagnostic::default()
            }
        })
        .collect()
}

/// The "machine says" hover section for the entries covering a prompt line.
#[must_use]
pub fn ml_hover_markdown(entries: &[&MlEntry]) -> String {
    let mut lines: Vec<String> = vec!["**machine says**".to_string(), String::new()];
    for entry in entries {
        lines.push(format!("- _{}_ — {}", kind_label(entry.kind), entry.message));
        if !entry.reasoning.trim().is_empty() {
            lines.push(format!("  {}", entry.reasoning));
        }
        if let Some(suggestion) = &entry.suggestion {
            if !suggestion.trim().is_empty() {
                lines.push("```".to_string());
                lines.push(suggestion.clone());
                lines.push("```".to_string());
            }
        }
    }
    lines.join("\n")
}

fn kind_label(kind: MlKind) -> &'static str {
    match kind {
        MlKind::Ambiguity => "ambiguity",
        MlKind::Assumption => "assumption",
        MlKind::Note => "note",
        MlKind::NoOp => "no-op",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use napl_core::schemas::validate_ml;
    use serde_json::json;

    #[test]
    fn maps_each_kind_to_intended_severity() {
        assert_eq!(ml_severity(MlKind::Ambiguity), DiagnosticSeverity::ERROR);
        assert_eq!(ml_severity(MlKind::Assumption), DiagnosticSeverity::WARNING);
        assert_eq!(ml_severity(MlKind::NoOp), DiagnosticSeverity::WARNING);
        assert_eq!(ml_severity(MlKind::Note), DiagnosticSeverity::INFORMATION);
    }

    #[test]
    fn places_diagnostics_converting_body_relative_lines() {
        let ml = validate_ml(json!({
            "module": "todo-app",
            "target": "react",
            "entries": [
                { "promptLines": [3, 3], "kind": "ambiguity", "message": "vague phrase", "reasoning": "r" },
                { "promptLines": [1, 2], "kind": "assumption", "message": "assumed default" }
            ]
        }))
        .unwrap();
        let doc_lines = vec![
            "---",
            "module: x",
            "---",
            "body line 1",
            "body line 2",
            "body line 3",
        ];
        let diagnostics = ml_diagnostics(&ml, 3, &doc_lines);
        assert_eq!(diagnostics[0].severity, Some(DiagnosticSeverity::ERROR));
        assert_eq!(diagnostics[0].source.as_deref(), Some("napl-mapl"));
        assert_eq!(diagnostics[0].message, "vague phrase");
        assert_eq!(diagnostics[0].range.start.line, 5);
        assert_eq!(diagnostics[0].range.end.line, 5);
        assert_eq!(diagnostics[0].range.end.character, "body line 3".len() as u32);
        assert_eq!(diagnostics[1].severity, Some(DiagnosticSeverity::WARNING));
        assert_eq!(diagnostics[1].range.start.line, 3);
        assert_eq!(diagnostics[1].range.end.line, 4);
    }

    #[test]
    fn renders_machine_says_section() {
        let ml = validate_ml(json!({
            "module": "m", "target": "react",
            "entries": [
                { "promptLines": [1, 1], "kind": "ambiguity", "message": "odd literal", "reasoning": "why it is odd", "suggestion": "reword to X" }
            ]
        }))
        .unwrap();
        let refs: Vec<&MlEntry> = ml.entries.iter().collect();
        let md = ml_hover_markdown(&refs);
        assert!(md.contains("**machine says**"));
        assert!(md.contains("_ambiguity_ — odd literal"));
        assert!(md.contains("why it is odd"));
        assert!(md.contains("```\nreword to X\n```"));
    }

    #[test]
    fn omits_suggestion_fence_when_absent() {
        let ml = validate_ml(json!({
            "module": "m", "target": "react",
            "entries": [{ "promptLines": [1, 1], "kind": "note", "message": "m", "reasoning": "" }]
        }))
        .unwrap();
        let refs: Vec<&MlEntry> = ml.entries.iter().collect();
        let md = ml_hover_markdown(&refs);
        assert!(md.contains("_note_ — m"));
        assert!(!md.contains("```"));
    }
}
