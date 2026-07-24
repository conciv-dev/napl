//! Equivalence gate for the `cmd_watch` module's pure ignore-predicate slice.
//!
//! Replays the EXACT unit-test corpus of the hand-written `napl-cli` `cmd_watch`
//! module (rust/crates/napl-cli/src/cmd_watch.rs — the `tests` module) against
//! the NAPL-generated `watch_filter` crate.

use std::path::Path;

use watch_filter::is_ignored;

#[test]
fn ignores_paths_under_toolchain_and_vcs_dirs() {
    let root = Path::new("/proj");
    assert!(is_ignored(&root.join("node_modules/dep.js"), root));
    assert!(is_ignored(&root.join(".napl/src/rust/x.rs"), root));
    assert!(is_ignored(&root.join(".git/HEAD"), root));
    assert!(is_ignored(&root.join("src/a/.napl/b"), root));
}

#[test]
fn keeps_ordinary_prompt_paths() {
    let root = Path::new("/proj");
    assert!(!is_ignored(&root.join("examples/greeting.napl"), root));
    assert!(!is_ignored(&root.join("src/lib.rs"), root));
}
