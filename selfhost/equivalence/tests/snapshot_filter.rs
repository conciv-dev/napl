//! Equivalence gate for the `snapshot` module's pure exclusion-filter slice.
//!
//! Replays the EXACT unit-test corpus of the hand-written `napl-cli` `snapshot`
//! module (rust/crates/napl-cli/src/snapshot.rs — the
//! `filter_predicate_decides_dirs_files_roots_and_suffixes` test) against the
//! NAPL-generated `snapshot_filter` crate.

use snapshot_filter::make_filter;

#[test]
fn filter_predicate_decides_dirs_files_roots_and_suffixes() {
    let filter = make_filter(
        &["node_modules".to_string(), ".git".to_string()],
        &["AGENTS.md".to_string()],
        &["Cargo.toml".to_string()],
        &[".d.ts".to_string(), ".lock".to_string()],
    );
    assert!(filter.is_excluded_dir("node_modules"));
    assert!(filter.is_excluded_dir(".git"));
    assert!(!filter.is_excluded_dir("src"));
    assert!(filter.is_excluded_file("AGENTS.md", false));
    assert!(filter.is_excluded_file("AGENTS.md", true));
    assert!(filter.is_excluded_file("types.d.ts", false));
    assert!(filter.is_excluded_file("Cargo.lock", true));
    assert!(!filter.is_excluded_file("keep.ts", false));
    assert!(filter.is_excluded_file("Cargo.toml", true));
    assert!(!filter.is_excluded_file("Cargo.toml", false));
}
