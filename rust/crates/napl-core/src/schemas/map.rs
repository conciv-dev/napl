//! NAPL map v2 schema and its pure query/mutation helpers.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use super::ordered_map::OrderedMap;
use super::SchemaError;

fn default_version() -> u32 {
    2
}

/// A per-target record inside a prompt entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PromptTargetRecord {
    #[serde(
        rename = "promptHashAtGen",
        skip_serializing_if = "Option::is_none",
        default
    )]
    pub prompt_hash_at_gen: Option<String>,
    #[serde(default)]
    pub files: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub unattributed: Option<bool>,
}

/// A prompt entry in the map.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PromptRecord {
    pub module: String,
    #[serde(rename = "promptHash")]
    pub prompt_hash: String,
    #[serde(rename = "declaredTargets", default)]
    pub declared_targets: Vec<String>,
    #[serde(default)]
    pub targets: OrderedMap<PromptTargetRecord>,
}

/// A file entry in the map.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileRecord {
    pub target: String,
    pub hash: String,
    #[serde(default)]
    pub prompts: Vec<String>,
}

/// The whole v2 map.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NaplMap {
    #[serde(default = "default_version")]
    pub version: u32,
    #[serde(default)]
    pub prompts: OrderedMap<PromptRecord>,
    #[serde(default)]
    pub files: OrderedMap<FileRecord>,
}

/// A `(target, filePath)` pair produced by [`files_for_module`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleFile {
    pub target: String,
    pub file_path: String,
}

/// A `(filePath, hash)` pair for [`record_attribution`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileInput {
    pub file_path: String,
    pub hash: String,
}

/// Input for [`record_attribution`].
#[derive(Debug, Clone)]
pub struct AttributionInput {
    pub rel: String,
    pub module: String,
    pub prompt_hash: String,
    pub target: String,
    pub declared_targets: Vec<String>,
    pub files: Vec<FileInput>,
}

/// Input for [`record_unattributed`].
#[derive(Debug, Clone)]
pub struct UnattributedInput {
    pub rel: String,
    pub module: String,
    pub prompt_hash: String,
    pub target: String,
    pub declared_targets: Vec<String>,
    pub files: Vec<String>,
}

/// An empty v2 map.
#[must_use]
pub fn empty_map() -> NaplMap {
    NaplMap {
        version: 2,
        prompts: OrderedMap::new(),
        files: OrderedMap::new(),
    }
}

/// Parse and validate a map JSON string, mirroring `parseMap`.
pub fn parse_map(raw: &str) -> Result<NaplMap, SchemaError> {
    let map: NaplMap =
        serde_json::from_str(raw).map_err(|e| SchemaError::Deserialize(e.to_string()))?;
    if map.version != 2 {
        return Err(SchemaError::Validation(format!(
            "unsupported map version {}",
            map.version
        )));
    }
    Ok(map)
}

/// Serialize a map the way `writeMap` does: pretty-printed with a trailing newline.
#[must_use]
pub fn map_to_json(map: &NaplMap) -> String {
    let mut out = serde_json::to_string_pretty(map).expect("map serializes");
    out.push('\n');
    out
}

/// Relative prompt paths belonging to `module`, in insertion order.
#[must_use]
pub fn prompts_for_module(map: &NaplMap, module: &str) -> Vec<String> {
    map.prompts
        .iter()
        .filter(|(_, record)| record.module == module)
        .map(|(rel, _)| rel.clone())
        .collect()
}

/// Whether the module has any prompts.
#[must_use]
pub fn has_module(map: &NaplMap, module: &str) -> bool {
    !prompts_for_module(map, module).is_empty()
}

/// Declared targets across a module's prompts, deduplicated in insertion order.
#[must_use]
pub fn declared_targets_for_module(map: &NaplMap, module: &str) -> Vec<String> {
    let mut seen: HashSet<String> = HashSet::new();
    let mut out: Vec<String> = Vec::new();
    for rel in prompts_for_module(map, module) {
        if let Some(record) = map.prompts.get(&rel) {
            for target in &record.declared_targets {
                if seen.insert(target.clone()) {
                    out.push(target.clone());
                }
            }
        }
    }
    out
}

/// Files produced by a module, deduplicated by `(target, filePath)`.
#[must_use]
pub fn files_for_module(map: &NaplMap, module: &str) -> Vec<ModuleFile> {
    let mut seen: HashSet<String> = HashSet::new();
    let mut files: Vec<ModuleFile> = Vec::new();
    for rel in prompts_for_module(map, module) {
        let Some(record) = map.prompts.get(&rel) else {
            continue;
        };
        for (target, target_record) in record.targets.iter() {
            for file_path in &target_record.files {
                let key = format!("{target} {file_path}");
                if !seen.insert(key) {
                    continue;
                }
                files.push(ModuleFile {
                    target: target.clone(),
                    file_path: file_path.clone(),
                });
            }
        }
    }
    files
}

/// Whether a prompt/target generation is stale, mirroring `isPromptGenStale`.
#[must_use]
pub fn is_prompt_gen_stale(
    record: Option<&PromptRecord>,
    target: &str,
    prompt_hash: &str,
    force: bool,
) -> bool {
    if force {
        return true;
    }
    let Some(record) = record else {
        return true;
    };
    let Some(target_record) = record.targets.get(target) else {
        return true;
    };
    if target_record.unattributed == Some(true) {
        return true;
    }
    match &target_record.prompt_hash_at_gen {
        None => true,
        Some(hash) => hash != prompt_hash,
    }
}

fn detach_file(map: &mut NaplMap, file_path: &str, rel: &str) {
    let Some(record) = map.files.get(file_path) else {
        return;
    };
    let remaining: Vec<String> = record
        .prompts
        .iter()
        .filter(|prompt| *prompt != rel)
        .cloned()
        .collect();
    if remaining.is_empty() {
        map.files.remove(file_path);
    } else {
        let mut updated = record.clone();
        updated.prompts = remaining;
        map.files.insert(file_path.to_string(), updated);
    }
}

/// Record a successful attribution, mirroring `recordAttribution`.
pub fn record_attribution(map: &mut NaplMap, input: &AttributionInput) {
    let existing_targets = map.prompts.get(&input.rel).map(|r| r.targets.clone());
    let prior_files: Vec<String> = existing_targets
        .as_ref()
        .and_then(|t| t.get(&input.target))
        .map(|tr| tr.files.clone())
        .unwrap_or_default();
    let next_paths: HashSet<&str> = input.files.iter().map(|f| f.file_path.as_str()).collect();

    for file_path in &prior_files {
        if next_paths.contains(file_path.as_str()) {
            continue;
        }
        detach_file(map, file_path, &input.rel);
    }

    let mut targets = existing_targets.unwrap_or_default();
    targets.insert(
        input.target.clone(),
        PromptTargetRecord {
            prompt_hash_at_gen: Some(input.prompt_hash.clone()),
            files: input.files.iter().map(|f| f.file_path.clone()).collect(),
            unattributed: None,
        },
    );
    map.prompts.insert(
        input.rel.clone(),
        PromptRecord {
            module: input.module.clone(),
            prompt_hash: input.prompt_hash.clone(),
            declared_targets: input.declared_targets.clone(),
            targets,
        },
    );

    for f in &input.files {
        let mut prompts: Vec<String> = map
            .files
            .get(&f.file_path)
            .map(|r| r.prompts.clone())
            .unwrap_or_default();
        if !prompts.iter().any(|p| p == &input.rel) {
            prompts.push(input.rel.clone());
        }
        map.files.insert(
            f.file_path.clone(),
            FileRecord {
                target: input.target.clone(),
                hash: f.hash.clone(),
                prompts,
            },
        );
    }
}

/// Record an unattributed generation, mirroring `recordUnattributed`.
pub fn record_unattributed(map: &mut NaplMap, input: &UnattributedInput) {
    let existing_targets = map.prompts.get(&input.rel).map(|r| r.targets.clone());
    let prior_files: Vec<String> = existing_targets
        .as_ref()
        .and_then(|t| t.get(&input.target))
        .map(|tr| tr.files.clone())
        .unwrap_or_default();

    for file_path in &prior_files {
        detach_file(map, file_path, &input.rel);
    }

    let mut targets = existing_targets.unwrap_or_default();
    targets.insert(
        input.target.clone(),
        PromptTargetRecord {
            prompt_hash_at_gen: None,
            files: input.files.clone(),
            unattributed: Some(true),
        },
    );
    map.prompts.insert(
        input.rel.clone(),
        PromptRecord {
            module: input.module.clone(),
            prompt_hash: input.prompt_hash.clone(),
            declared_targets: input.declared_targets.clone(),
            targets,
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seeded() -> NaplMap {
        let mut map = empty_map();
        record_attribution(
            &mut map,
            &AttributionInput {
                rel: "examples/greeting.napl".to_string(),
                module: "greeting".to_string(),
                prompt_hash: "p1".to_string(),
                target: "typescript".to_string(),
                declared_targets: vec!["typescript".to_string()],
                files: vec![
                    FileInput {
                        file_path: ".napl/src/typescript/greeting.ts".to_string(),
                        hash: "h1".to_string(),
                    },
                    FileInput {
                        file_path: ".napl/src/typescript/greeting.test.ts".to_string(),
                        hash: "h2".to_string(),
                    },
                ],
            },
        );
        map
    }

    #[test]
    fn empty_is_v2() {
        assert_eq!(empty_map().version, 2);
        assert!(empty_map().prompts.is_empty());
    }

    #[test]
    fn round_trips_written_map() {
        let map = seeded();
        let json = map_to_json(&map);
        assert!(json.ends_with('\n'));
        let back = parse_map(&json).unwrap();
        assert_eq!(back, map);
    }

    #[test]
    fn throws_on_corrupt_json() {
        assert!(parse_map("{ not json").is_err());
    }

    #[test]
    fn applies_defaults_for_sparse_prompt_records() {
        let map =
            parse_map(r#"{"version":2,"prompts":{"a.napl":{"module":"m","promptHash":"h"}}}"#)
                .unwrap();
        let record = map.prompts.get("a.napl").unwrap();
        assert!(record.declared_targets.is_empty());
        assert!(record.targets.is_empty());
    }

    #[test]
    fn resolves_membership_targets_and_files() {
        let map = seeded();
        assert!(has_module(&map, "greeting"));
        assert!(!has_module(&map, "missing"));
        assert_eq!(
            declared_targets_for_module(&map, "greeting"),
            vec!["typescript"]
        );
        assert_eq!(
            files_for_module(&map, "greeting"),
            vec![
                ModuleFile {
                    target: "typescript".to_string(),
                    file_path: ".napl/src/typescript/greeting.ts".to_string()
                },
                ModuleFile {
                    target: "typescript".to_string(),
                    file_path: ".napl/src/typescript/greeting.test.ts".to_string()
                },
            ]
        );
    }

    #[test]
    fn records_many_to_many_file_attribution() {
        let mut map = seeded();
        record_attribution(
            &mut map,
            &AttributionInput {
                rel: "examples/extra.napl".to_string(),
                module: "extra".to_string(),
                prompt_hash: "p2".to_string(),
                target: "typescript".to_string(),
                declared_targets: vec!["typescript".to_string()],
                files: vec![FileInput {
                    file_path: ".napl/src/typescript/greeting.ts".to_string(),
                    hash: "h1".to_string(),
                }],
            },
        );
        let mut prompts = map
            .files
            .get(".napl/src/typescript/greeting.ts")
            .unwrap()
            .prompts
            .clone();
        prompts.sort();
        assert_eq!(
            prompts,
            vec!["examples/extra.napl", "examples/greeting.napl"]
        );
    }

    #[test]
    fn drops_orphaned_file_attributions() {
        let mut map = seeded();
        record_attribution(
            &mut map,
            &AttributionInput {
                rel: "examples/greeting.napl".to_string(),
                module: "greeting".to_string(),
                prompt_hash: "p1b".to_string(),
                target: "typescript".to_string(),
                declared_targets: vec!["typescript".to_string()],
                files: vec![FileInput {
                    file_path: ".napl/src/typescript/greeting.ts".to_string(),
                    hash: "h1b".to_string(),
                }],
            },
        );
        assert!(map
            .files
            .get(".napl/src/typescript/greeting.test.ts")
            .is_none());
        assert_eq!(
            map.prompts
                .get("examples/greeting.napl")
                .unwrap()
                .targets
                .get("typescript")
                .unwrap()
                .files,
            vec![".napl/src/typescript/greeting.ts"]
        );
    }

    #[test]
    fn is_prompt_gen_stale_cases() {
        let map = seeded();
        let record = map.prompts.get("examples/greeting.napl");
        assert!(!is_prompt_gen_stale(record, "typescript", "p1", false));
        assert!(is_prompt_gen_stale(record, "typescript", "p2", false));
        assert!(is_prompt_gen_stale(record, "react", "p1", false));
        assert!(is_prompt_gen_stale(None, "typescript", "p1", false));
        assert!(is_prompt_gen_stale(record, "typescript", "p1", true));
    }

    #[test]
    fn unattributed_marks_target_and_detaches() {
        let mut map = seeded();
        record_unattributed(
            &mut map,
            &UnattributedInput {
                rel: "examples/greeting.napl".to_string(),
                module: "greeting".to_string(),
                prompt_hash: "p1".to_string(),
                target: "typescript".to_string(),
                declared_targets: vec!["typescript".to_string()],
                files: vec![".napl/src/typescript/greeting.ts".to_string()],
            },
        );
        let entry = map
            .prompts
            .get("examples/greeting.napl")
            .unwrap()
            .targets
            .get("typescript")
            .unwrap();
        assert_eq!(entry.unattributed, Some(true));
        assert_eq!(entry.prompt_hash_at_gen, None);
        assert_eq!(entry.files, vec![".napl/src/typescript/greeting.ts"]);
        assert!(map.files.get(".napl/src/typescript/greeting.ts").is_none());
        assert!(map
            .files
            .get(".napl/src/typescript/greeting.test.ts")
            .is_none());

        assert!(is_prompt_gen_stale(
            map.prompts.get("examples/greeting.napl"),
            "typescript",
            "p1",
            false
        ));
    }

    #[test]
    fn unattributed_cleared_by_subsequent_attribution() {
        let mut map = seeded();
        record_unattributed(
            &mut map,
            &UnattributedInput {
                rel: "examples/greeting.napl".to_string(),
                module: "greeting".to_string(),
                prompt_hash: "p1".to_string(),
                target: "typescript".to_string(),
                declared_targets: vec!["typescript".to_string()],
                files: vec![".napl/src/typescript/greeting.ts".to_string()],
            },
        );
        record_attribution(
            &mut map,
            &AttributionInput {
                rel: "examples/greeting.napl".to_string(),
                module: "greeting".to_string(),
                prompt_hash: "p2".to_string(),
                target: "typescript".to_string(),
                declared_targets: vec!["typescript".to_string()],
                files: vec![FileInput {
                    file_path: ".napl/src/typescript/greeting.ts".to_string(),
                    hash: "h9".to_string(),
                }],
            },
        );
        let entry = map
            .prompts
            .get("examples/greeting.napl")
            .unwrap()
            .targets
            .get("typescript")
            .unwrap();
        assert_eq!(entry.unattributed, None);
        assert_eq!(entry.prompt_hash_at_gen, Some("p2".to_string()));
    }
}
