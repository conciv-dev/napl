//! Stage1 adapter over the NAPL-generated `schemas_map` crate. Its parse error
//! is a `String`, mapped to the shared `SchemaError`.

use super::SchemaError;

pub use schemas_map::{
    declared_targets_for_module, empty_map, files_for_module, has_module, is_prompt_gen_stale,
    map_to_json, prompts_for_module, record_attribution, record_unattributed, AttributionInput,
    FileInput, FileRecord, ModuleFile, NaplMap, PromptRecord, PromptTargetRecord, UnattributedInput,
};

pub fn parse_map(raw: &str) -> Result<NaplMap, SchemaError> {
    schemas_map::parse_map(raw).map_err(SchemaError::Deserialize)
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
