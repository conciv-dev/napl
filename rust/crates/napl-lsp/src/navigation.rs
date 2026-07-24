//! Definition, references, and CodeLens. Definition and references work both
//! directions: a prompt line resolves to its generated source location(s), and a
//! generated line resolves back to the prompt sentence(s) that own it. CodeLens
//! annotates generated files with the owning prompt, matching the TS LSP.

use std::path::Path;

use tower_lsp_server::ls_types::{
    CodeLens, Command, GotoDefinitionResponse, Location, Position,
};

use napl_core::hash::content_hash;
use napl_core::reverse::{
    code_lens_title, dedupe_matches, is_file_drifted, reverse_matches, ReverseMatch,
    DRIFT_LENS_PREFIX,
};
use napl_core::schemas::{
    entries_at_body_line, files_for_module, has_module, ModuleFile,
};
use napl_core::scanner::{find_target_at_position, scan_document, Target};

use crate::context::{
    mechanical_label, ordered_files, prompt_location, resolve_body_context,
    resolve_generated_context, GeneratedContext,
};
use crate::convert::{range, split_lines, uri_for_path, utf16_len};
use crate::state::{find_workspace_root, load_attribution, load_mechanical, read_map};

/// Resolve the definition(s) at a position.
#[must_use]
pub fn definition(
    doc_path: &Path,
    doc_text: &str,
    position: Position,
) -> Option<GotoDefinitionResponse> {
    let root = find_workspace_root(doc_path)?;

    if let Some(ctx) = resolve_generated_context(&root, doc_path) {
        return Some(GotoDefinitionResponse::Array(reverse_definition(
            &root,
            &ctx,
            position.line as usize,
        )));
    }

    let scan = scan_document(doc_text);
    let scan_pos = napl_core::scanner::Position {
        line: position.line as usize,
        character: position.character as usize,
    };
    if let Some(target) = find_target_at_position(&scan, scan_pos) {
        let module = target_module(&target);
        let map = read_map(&root);
        if !has_module(&map, module) {
            return None;
        }
        let mut locations = Vec::new();
        for file in ordered_files(files_for_module(&map, module), |f: &ModuleFile| {
            f.file_path.as_str()
        }) {
            let abs = root.join(&file.file_path);
            if !abs.exists() {
                continue;
            }
            if let Some(uri) = uri_for_path(&abs) {
                locations.push(Location {
                    uri,
                    range: range(0, 0, 0, 0),
                });
            }
        }
        return Some(GotoDefinitionResponse::Array(locations));
    }

    let context = resolve_body_context(doc_text, position.line)?;
    let attribution = load_attribution(&root, &context.module)?;
    let entries = entries_at_body_line(&attribution, context.body_line);
    if entries.is_empty() {
        return None;
    }
    let mut locations = Vec::new();
    for entry in entries {
        let abs = root
            .join(".napl")
            .join("src")
            .join(&attribution.target)
            .join(&entry.file);
        let Ok(code) = std::fs::read_to_string(&abs) else {
            continue;
        };
        let code_lines = split_lines(&code);
        let end_index = (entry.lines.end as usize).min(code_lines.len()).max(1) - 1;
        let end_char = code_lines.get(end_index).map_or(0, |line| utf16_len(line));
        if let Some(uri) = uri_for_path(&abs) {
            locations.push(Location {
                uri,
                range: range(entry.lines.start as usize - 1, 0, end_index, end_char),
            });
        }
    }
    Some(GotoDefinitionResponse::Array(locations))
}

/// Resolve references — only the generated→prompt direction, matching the TS LSP.
#[must_use]
pub fn references(doc_path: &Path, position: Position) -> Option<Vec<Location>> {
    let root = find_workspace_root(doc_path)?;
    let ctx = resolve_generated_context(&root, doc_path)?;
    reverse_references(&root, &ctx, position.line as usize)
}

/// CodeLenses for a generated file: one per owning prompt sentence.
#[must_use]
pub fn code_lens(doc_path: &Path, doc_text: &str) -> Vec<CodeLens> {
    let Some(root) = find_workspace_root(doc_path) else {
        return Vec::new();
    };
    let Some(ctx) = resolve_generated_context(&root, doc_path) else {
        return Vec::new();
    };
    reverse_code_lenses(&root, &ctx, doc_text)
}

fn target_module(target: &Target) -> &str {
    match target {
        Target::ModuleValue { module, .. }
        | Target::Dep { module, .. }
        | Target::Ref { module, .. } => module,
    }
}

fn reverse_definition(root: &Path, ctx: &GeneratedContext, line: usize) -> Vec<Location> {
    let matches = dedupe_matches(&reverse_matches(
        &ctx.sources,
        &ctx.info.target,
        &ctx.info.target_rel_path,
        Some(line as u32 + 1),
    ));
    let mut locations: Vec<Location> = matches
        .iter()
        .filter_map(|m| prompt_location(root, m))
        .collect();
    if locations.is_empty() {
        locations = file_level_prompt_fallback(root, ctx);
    }
    locations
}

fn file_level_prompt_fallback(root: &Path, ctx: &GeneratedContext) -> Vec<Location> {
    let Some(record) = ctx.map.files.get(&ctx.rel_full) else {
        return Vec::new();
    };
    let mut locations = Vec::new();
    for prompt_file in &record.prompts {
        let abs = root.join(prompt_file);
        if !abs.exists() {
            continue;
        }
        if let Some(uri) = uri_for_path(&abs) {
            locations.push(Location {
                uri,
                range: range(0, 0, 0, 0),
            });
        }
    }
    locations
}

fn reverse_references(root: &Path, ctx: &GeneratedContext, line: usize) -> Option<Vec<Location>> {
    let mut matches = reverse_matches(
        &ctx.sources,
        &ctx.info.target,
        &ctx.info.target_rel_path,
        Some(line as u32 + 1),
    );
    if matches.is_empty() {
        matches = reverse_matches(&ctx.sources, &ctx.info.target, &ctx.info.target_rel_path, None);
    }
    let locations: Vec<Location> = dedupe_matches(&matches)
        .iter()
        .filter_map(|m| prompt_location(root, m))
        .collect();
    if locations.is_empty() {
        None
    } else {
        Some(locations)
    }
}

fn reverse_code_lenses(root: &Path, ctx: &GeneratedContext, current_text: &str) -> Vec<CodeLens> {
    let matches = reverse_matches(&ctx.sources, &ctx.info.target, &ctx.info.target_rel_path, None);
    if matches.is_empty() {
        return Vec::new();
    }
    let recorded_hash = ctx.map.files.get(&ctx.rel_full).map(|f| f.hash.as_str());
    let drifted = is_file_drifted(recorded_hash, &content_hash(current_text));
    let mechanical = load_mechanical(root, &ctx.info);

    let mut sorted: Vec<ReverseMatch> = matches;
    sorted.sort_by_key(|m| m.code_lines.start);

    let mut lenses = Vec::new();
    let mut drift_applied = false;
    for m in &sorted {
        let Some(location) = prompt_location(root, m) else {
            continue;
        };
        let semantic = code_lens_title(
            basename(&m.prompt_file),
            location.range.start.line as usize + 1,
            &m.note,
        );
        let mech_label = mechanical
            .as_ref()
            .and_then(|mech| mechanical_label(mech, m.code_lines.start as usize));
        let base = match mech_label {
            Some(label) => format!("{label}   {semantic}"),
            None => semantic,
        };
        let title = if drifted && !drift_applied {
            format!("{DRIFT_LENS_PREFIX}   {base}")
        } else {
            base
        };
        if drifted {
            drift_applied = true;
        }
        let anchor = m.code_lines.start as usize - 1;
        let arguments = vec![
            serde_json::Value::String(location.uri.as_str().to_string()),
            serde_json::to_value(location.range).unwrap_or(serde_json::Value::Null),
        ];
        lenses.push(CodeLens {
            range: range(anchor, 0, anchor, 0),
            command: Some(Command {
                title,
                command: "napl.revealLocation".to_string(),
                arguments: Some(arguments),
            }),
            data: None,
        });
    }
    lenses
}

fn basename(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}
