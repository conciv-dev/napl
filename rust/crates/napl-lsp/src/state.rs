//! Disk readers for the `.napl/` state the LSP consults: the map, attribution,
//! machine-layer, IR, and journal. Parsing and validation are delegated to
//! `napl-core`; this module only performs the file I/O and the YAML→JSON bridge.

use std::path::{Path, PathBuf};

use napl_core::blame::{blame_file, BlameLine};
use napl_core::extensions::machine_extensions;
use napl_core::reverse::{AttributionSource, GeneratedPathInfo};
use napl_core::schemas::{
    empty_map, file_history, prompts_for_module, read_journal_str, validate_attribution,
    validate_ir, validate_ml, Attribution, Ir, Ml, NaplMap,
};

/// Walk up from `start` until a directory containing `.napl` is found.
#[must_use]
pub fn find_workspace_root(start: &Path) -> Option<PathBuf> {
    let mut dir = start.parent();
    while let Some(current) = dir {
        if current.join(".napl").is_dir() {
            return Some(current.to_path_buf());
        }
        dir = current.parent();
    }
    None
}

fn yaml_to_json(raw: &str) -> Option<serde_json::Value> {
    let value: serde_yaml::Value = serde_yaml::from_str(raw).ok()?;
    serde_json::to_value(value).ok()
}

/// Read and parse the map, tolerating a missing or corrupt file by returning an
/// empty map so the server never fails a request over bad state.
#[must_use]
pub fn read_map(root: &Path) -> NaplMap {
    let path = root.join(".napl").join("map.json");
    match std::fs::read_to_string(path) {
        Ok(raw) => napl_core::schemas::parse_map(&raw).unwrap_or_else(|_| empty_map()),
        Err(_) => empty_map(),
    }
}

/// Load a module's attribution document, or `None` when absent or invalid.
#[must_use]
pub fn load_attribution(root: &Path, module: &str) -> Option<Attribution> {
    let path = root
        .join(".napl")
        .join("attribution")
        .join(format!("{module}.yaml"));
    let raw = std::fs::read_to_string(path).ok()?;
    validate_attribution(yaml_to_json(&raw)?).ok()
}

/// Load a module's machine-layer document under either spelling, or `None`.
#[must_use]
pub fn load_ml(root: &Path, module: &str) -> Option<Ml> {
    for ext in machine_extensions() {
        let path = root
            .join(".napl")
            .join("mapl")
            .join(format!("{module}{ext}"));
        let Ok(raw) = std::fs::read_to_string(path) else {
            continue;
        };
        return validate_ml(yaml_to_json(&raw)?).ok();
    }
    None
}

/// Load a module's IR document, or `None` when absent or invalid.
#[must_use]
pub fn load_ir(root: &Path, module: &str) -> Option<Ir> {
    let path = root.join(".napl").join("ir").join(format!("{module}.yaml"));
    let raw = std::fs::read_to_string(path).ok()?;
    validate_ir(yaml_to_json(&raw)?).ok()
}

/// Every module's attribution, tagged with the prompts that contribute to it.
#[must_use]
pub fn load_attribution_sources(root: &Path, map: &NaplMap) -> Vec<AttributionSource> {
    let dir = root.join(".napl").join("attribution");
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut names: Vec<String> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| std::path::Path::new(name).extension().is_some_and(|ext| ext == "yaml"))
        .collect();
    names.sort();
    let mut sources = Vec::new();
    for name in names {
        let module = &name[..name.len() - ".yaml".len()];
        let Some(attribution) = load_attribution(root, module) else {
            continue;
        };
        let prompt_files = prompts_for_module(map, &attribution.module);
        sources.push(AttributionSource {
            module: attribution.module,
            target: attribution.target,
            entries: attribution.entries,
            prompt_files,
        });
    }
    sources
}

/// The blame result and prompt-diff-by-gen table for a generated file, or `None`
/// when it has no recorded journal history.
pub struct Mechanical {
    pub blamed: Vec<BlameLine>,
    pub prompt_diff_by_gen: std::collections::HashMap<i64, String>,
}

/// Load the blame/mechanical context for a generated file.
#[must_use]
pub fn load_mechanical(root: &Path, info: &GeneratedPathInfo) -> Option<Mechanical> {
    let journal_path = root.join(".napl").join("journal.jsonl");
    let raw = std::fs::read_to_string(journal_path).ok()?;
    let (entries, _warnings) = read_journal_str(&raw);
    if entries.is_empty() {
        return None;
    }
    let rel_path = format!(".napl/src/{}/{}", info.target, info.target_rel_path);
    let history = file_history(&entries, &rel_path);
    if history.is_empty() {
        return None;
    }
    let abs = root.join(&rel_path);
    let content = std::fs::read_to_string(abs).ok()?;
    let blamed = blame_file(&history, &content);
    let prompt_diff_by_gen = history
        .iter()
        .map(|entry| (entry.gen, entry.prompt_diff.clone()))
        .collect();
    Some(Mechanical {
        blamed,
        prompt_diff_by_gen,
    })
}
