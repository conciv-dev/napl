//! Hover: reverse hover on generated files (which prompt sentence produced this
//! line, and the gen that caused it), module/dep/ref hover on prompt tokens, and
//! attribution + machine-layer hover on prompt body lines.

use std::path::Path;

use tower_lsp_server::ls_types::{Hover, HoverContents, MarkupContent, MarkupKind, Position};

use napl_core::reverse::{dedupe_matches, prompt_absolute_lines, reverse_matches};
use napl_core::schemas::{
    declared_targets_for_module, entries_at_body_line, files_for_module, has_module,
    ml_entries_at_body_line, Attribution, AttributionEntry, ModuleFile,
};
use napl_core::scanner::{find_target_at_position, scan_document, Target};

use crate::context::{
    fence_lang, mechanical_label, ordered_files, resolve_body_context, resolve_generated_context,
    GeneratedContext,
};
use crate::convert::{range, scan_span, split_lines};
use crate::ml::ml_hover_markdown;
use crate::state::{find_workspace_root, load_attribution, load_ir, load_ml, load_mechanical};

const HOVER_CODE_LINES: usize = 40;

/// Compute the hover for a position in a NAPL-related document.
#[must_use]
pub fn hover(doc_path: &Path, doc_text: &str, position: Position) -> Option<Hover> {
    let root = find_workspace_root(doc_path)?;

    if let Some(ctx) = resolve_generated_context(&root, doc_path) {
        return reverse_hover(&root, &ctx, position.line as usize);
    }

    let scan = scan_document(doc_text);
    let scan_pos = napl_core::scanner::Position {
        line: position.line as usize,
        character: position.character as usize,
    };
    if let Some(target) = find_target_at_position(&scan, scan_pos) {
        let (module, span) = target_parts(&target);
        let markdown = build_hover_markdown(&root, module);
        return Some(Hover {
            contents: markdown_contents(markdown),
            range: Some(scan_span(span)),
        });
    }

    let context = resolve_body_context(doc_text, position.line)?;
    let attribution = load_attribution(&root, &context.module);
    let ml = load_ml(&root, &context.module);
    let attr_entries: Vec<&AttributionEntry> = attribution
        .as_ref()
        .map(|a| entries_at_body_line(a, context.body_line))
        .unwrap_or_default();
    let ml_entries = ml
        .as_ref()
        .map(|m| ml_entries_at_body_line(m, context.body_line))
        .unwrap_or_default();
    if attr_entries.is_empty() && ml_entries.is_empty() {
        return None;
    }

    let mut sections: Vec<String> = Vec::new();
    if let Some(attribution) = &attribution {
        if !attr_entries.is_empty() {
            if let Some(markdown) = build_attribution_hover(&root, attribution, &attr_entries) {
                sections.push(markdown);
            }
        }
    }
    if !ml_entries.is_empty() {
        sections.push(ml_hover_markdown(&ml_entries));
    }
    if sections.is_empty() {
        return None;
    }

    let body_start = napl_core::body_lines::prompt_body_lines(doc_text).body_start_line;
    let min_start = attr_entries
        .iter()
        .map(|e| e.prompt_lines.start)
        .chain(ml_entries.iter().map(|e| e.prompt_lines.start))
        .min()
        .unwrap_or(1);
    let max_end = attr_entries
        .iter()
        .map(|e| e.prompt_lines.end)
        .chain(ml_entries.iter().map(|e| e.prompt_lines.end))
        .max()
        .unwrap_or(1);
    let start_line = body_start + min_start as usize - 1;
    let end_line = body_start + max_end as usize - 1;
    Some(Hover {
        contents: markdown_contents(sections.join("\n\n---\n\n")),
        range: Some(range(start_line, 0, end_line, 200)),
    })
}

fn target_parts(target: &Target) -> (&str, napl_core::scanner::Span) {
    match target {
        Target::ModuleValue { module, span }
        | Target::Dep { module, span, .. }
        | Target::Ref { module, span } => (module, *span),
    }
}

fn markdown_contents(value: String) -> HoverContents {
    HoverContents::Markup(MarkupContent {
        kind: MarkupKind::Markdown,
        value,
    })
}

fn reverse_hover(root: &Path, ctx: &GeneratedContext, line: usize) -> Option<Hover> {
    let matches = reverse_matches(
        &ctx.sources,
        &ctx.info.target,
        &ctx.info.target_rel_path,
        Some(line as u32 + 1),
    );
    let mechanical = load_mechanical(root, &ctx.info);
    let mech_label = mechanical
        .as_ref()
        .and_then(|m| mechanical_label(m, line + 1));
    if matches.is_empty() && mech_label.is_none() {
        return None;
    }

    let mut blocks: Vec<String> = Vec::new();
    if let Some(label) = mech_label {
        blocks.push(format!("**{label}**"));
        blocks.push(String::new());
    }
    for m in dedupe_matches(&matches) {
        let abs = root.join(&m.prompt_file);
        let Ok(text) = std::fs::read_to_string(&abs) else {
            continue;
        };
        let body = napl_core::body_lines::prompt_body_lines(&text);
        let (start_line, end_line) = prompt_absolute_lines(body.body_start_line, m.prompt_lines);
        let doc_lines = split_lines(&text);
        let sentence = doc_lines
            .get(start_line..=end_line.min(doc_lines.len().saturating_sub(1)))
            .unwrap_or(&[])
            .join("\n");
        let base = basename(&m.prompt_file);
        let note = if m.note.is_empty() { "prompt" } else { &m.note };
        blocks.push(format!("**⇠ {base}:{}** — {note}", start_line + 1));
        blocks.push(
            sentence
                .split('\n')
                .map(|s| format!("> {s}"))
                .collect::<Vec<_>>()
                .join("\n"),
        );
        blocks.push(String::new());
    }
    if blocks.is_empty() {
        return None;
    }
    Some(Hover {
        contents: markdown_contents(blocks.join("\n")),
        range: None,
    })
}

fn build_hover_markdown(root: &Path, module: &str) -> String {
    let map = crate::state::read_map(root);
    if !has_module(&map, module) {
        return format!("**module `{module}`** — not generated yet — run `napl gen`.");
    }

    let mut lines: Vec<String> = Vec::new();
    lines.push(format!("### module `{module}`"));
    let declared = declared_targets_for_module(&map, module);
    let targets = if declared.is_empty() {
        "(none)".to_string()
    } else {
        declared.join(", ")
    };
    lines.push(format!("**targets:** {targets}"));

    if let Some(ir) = load_ir(root, module) {
        if !ir.functions.is_empty() {
            lines.push(String::new());
            lines.push("**signatures (IR)**".to_string());
            for function in &ir.functions {
                lines.push(format!("- `{}`", function.signature));
            }
        }
    }

    let files = ordered_files(files_for_module(&map, module), |f: &ModuleFile| {
        f.file_path.as_str()
    });
    let Some(implementation) = files.first() else {
        lines.push(String::new());
        lines.push("_no generated code yet — run `napl gen`._".to_string());
        return lines.join("\n");
    };

    let abs = root.join(&implementation.file_path);
    let Ok(code) = std::fs::read_to_string(&abs) else {
        lines.push(String::new());
        lines.push(format!(
            "_generated file missing: `{}` — run `napl gen`._",
            implementation.file_path
        ));
        return lines.join("\n");
    };
    let code_lines = split_lines(&code);
    let snippet = code_lines
        .iter()
        .take(HOVER_CODE_LINES)
        .copied()
        .collect::<Vec<_>>()
        .join("\n");
    let truncated = if code_lines.len() > HOVER_CODE_LINES {
        "\n…"
    } else {
        ""
    };
    lines.push(String::new());
    lines.push(format!(
        "**generated ({}) — `{}`**",
        implementation.target, implementation.file_path
    ));
    lines.push(format!("```{}", fence_lang(&implementation.file_path)));
    lines.push(format!("{snippet}{truncated}"));
    lines.push("```".to_string());
    lines.join("\n")
}

fn build_attribution_hover(
    root: &Path,
    attribution: &Attribution,
    entries: &[&AttributionEntry],
) -> Option<String> {
    let mut lines: Vec<String> = Vec::new();
    for entry in entries {
        let abs = root
            .join(".napl")
            .join("src")
            .join(&attribution.target)
            .join(&entry.file);
        let heading = if entry.note.is_empty() {
            "implemented by"
        } else {
            &entry.note
        };
        lines.push(format!(
            "**{heading}** — `{}` lines {}–{}",
            entry.file, entry.lines.start, entry.lines.end
        ));
        if let Ok(code) = std::fs::read_to_string(&abs) {
            let code_lines = split_lines(&code);
            let start = entry.lines.start as usize - 1;
            let end = (entry.lines.end as usize).min(code_lines.len());
            if start < end {
                let snippet = code_lines[start..end].join("\n");
                lines.push(format!("```{}", fence_lang(&entry.file)));
                lines.push(snippet);
                lines.push("```".to_string());
            }
        }
    }
    if lines.is_empty() {
        None
    } else {
        Some(lines.join("\n"))
    }
}

fn basename(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}
