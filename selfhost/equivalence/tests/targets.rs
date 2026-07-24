//! Equivalence gate for the `targets` module.
//!
//! This is the EXACT unit-test corpus of the hand-written `napl-core` `targets`
//! module (rust/crates/napl-core/src/targets.rs), replayed against the
//! NAPL-generated `targets` crate under selfhost/.napl/src/rust/targets/. Each
//! case asserts the same input -> output the hand-written module asserts for
//! itself, including the byte-exact user-facing labels, registry lists, and
//! error text.

use targets::{get_adapter, list_targets, starter_targets, workspace_manifest_toml};

#[test]
fn lists_registered_targets() {
    assert_eq!(list_targets(), vec!["typescript", "react", "rust"]);
}

#[test]
fn starter_targets_are_a_subset_of_registered() {
    assert_eq!(starter_targets(), vec!["typescript", "react"]);
    for target in starter_targets() {
        assert!(list_targets().contains(&target));
    }
}

#[test]
fn typescript_adapter_shape() {
    let adapter = get_adapter("typescript").unwrap();
    assert_eq!(adapter.name, "typescript");
    assert_eq!(adapter.test_command_label, "npx vitest run");
    assert_eq!(adapter.test_command("/x").args, vec!["vitest", "run"]);
    assert!(adapter.agent_tools.contains(&"Read".to_string()));
    assert!(!adapter
        .attribution_exclude_files
        .contains(&"vite.config.js".to_string()));
}

#[test]
fn react_adapter_excludes_vite_config_js() {
    let adapter = get_adapter("react").unwrap();
    assert!(adapter
        .attribution_exclude_files
        .contains(&"vite.config.js".to_string()));
}

#[test]
fn rust_adapter_shape() {
    let adapter = get_adapter("rust").unwrap();
    assert_eq!(adapter.name, "rust");
    assert_eq!(adapter.test_command_label, "cargo test");
    assert_eq!(adapter.test_command("/x").command, "cargo");
    assert_eq!(adapter.test_command("/x").args, vec!["test"]);
    assert!(adapter.agent_tools.contains(&"Bash(cargo:*)".to_string()));
    assert!(!adapter.agent_tools.contains(&"Bash(npx:*)".to_string()));
    assert!(adapter
        .attribution_exclude_dirs
        .contains(&"target".to_string()));
    assert!(adapter
        .attribution_exclude_files
        .contains(&"Cargo.lock".to_string()));
}

#[test]
fn rust_adapter_is_a_workspace_with_root_manifest_excluded() {
    let adapter = get_adapter("rust").unwrap();
    assert!(adapter.workspace_layout);
    assert!(adapter
        .attribution_exclude_root_files
        .contains(&"Cargo.toml".to_string()));
    assert!(adapter.idiom_guidance.contains("Cargo WORKSPACE"));
}

#[test]
fn single_package_targets_are_not_workspaces() {
    for name in ["typescript", "react"] {
        let adapter = get_adapter(name).unwrap();
        assert!(!adapter.workspace_layout);
        assert!(adapter.attribution_exclude_root_files.is_empty());
    }
}

#[test]
fn workspace_manifest_lists_members_deterministically() {
    let manifest = workspace_manifest_toml(&[
        "body_lines".to_string(),
        "extensions".to_string(),
        "hash".to_string(),
    ]);
    assert!(manifest.contains("[workspace]"));
    assert!(manifest.contains("resolver = \"2\""));
    assert!(manifest.contains("\"body_lines\","));
    assert!(manifest.contains("\"extensions\","));
    assert!(manifest.contains("\"hash\","));
    assert_eq!(
        manifest,
        workspace_manifest_toml(&[
            "body_lines".to_string(),
            "extensions".to_string(),
            "hash".to_string(),
        ])
    );
}

#[test]
fn unknown_target_errors() {
    let err = get_adapter("cobol").unwrap_err();
    assert!(err.contains("unknown target 'cobol'"));
    assert!(err.contains("typescript, react, rust"));
}
