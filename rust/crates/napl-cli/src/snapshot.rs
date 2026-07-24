//! Directory snapshots (hashes and contents) and their diff, mirroring
//! `snapshot.ts`.
//!
//! Stage1: the pure snapshot comparison (`diff_snapshots`) is the NAPL-generated
//! `snapshot_diff` crate and the pure exclusion filter (`SnapshotFilter`,
//! `make_filter`) is the NAPL-generated `snapshot_filter` crate, both re-exported
//! here; this shell keeps the filesystem walk. The unit corpus below rides along
//! as the regression net.

use std::collections::BTreeMap;
use std::path::Path;

use napl_core::hash::content_hash;

use crate::error::CliResult;

pub use snapshot_diff::diff_snapshots;
pub use snapshot_filter::{make_filter, SnapshotFilter};

fn walk(
    current: &Path,
    filter: &SnapshotFilter,
    with_content: bool,
    at_root: bool,
    out: &mut BTreeMap<String, String>,
) -> CliResult<()> {
    let Ok(entries) = std::fs::read_dir(current) else {
        return Ok(());
    };
    for entry in entries {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        let full = entry.path();
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            if filter.is_excluded_dir(&name) {
                continue;
            }
            walk(&full, filter, with_content, false, out)?;
        } else if file_type.is_file() {
            if filter.is_excluded_file(&name, at_root) {
                continue;
            }
            let content = std::fs::read_to_string(&full)?;
            let key = full.to_string_lossy().into_owned();
            if with_content {
                out.insert(key, content);
            } else {
                out.insert(key, content_hash(&content));
            }
        }
    }
    Ok(())
}

/// Snapshot the content hashes of a tree, mirroring `snapshotHashes`.
pub fn snapshot_hashes(dir: &Path, filter: &SnapshotFilter) -> CliResult<BTreeMap<String, String>> {
    let mut out = BTreeMap::new();
    walk(dir, filter, false, true, &mut out)?;
    Ok(out)
}

/// Snapshot the contents of a tree, mirroring `snapshotContents`.
pub fn snapshot_contents(
    dir: &Path,
    filter: &SnapshotFilter,
) -> CliResult<BTreeMap<String, String>> {
    let mut out = BTreeMap::new();
    walk(dir, filter, true, true, &mut out)?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diff_reports_added_and_changed() {
        let mut before = BTreeMap::new();
        before.insert("/a".to_string(), "h1".to_string());
        before.insert("/b".to_string(), "h2".to_string());
        let mut after = BTreeMap::new();
        after.insert("/a".to_string(), "h1".to_string());
        after.insert("/b".to_string(), "h2x".to_string());
        after.insert("/c".to_string(), "h3".to_string());
        assert_eq!(
            diff_snapshots(&before, &after),
            vec!["/b".to_string(), "/c".to_string()]
        );
    }

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

    #[test]
    fn filter_excludes_dirs_files_and_suffixes() {
        let dir = std::env::temp_dir().join(format!("napl-snap-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("node_modules")).unwrap();
        std::fs::write(dir.join("keep.ts"), "x").unwrap();
        std::fs::write(dir.join("AGENTS.md"), "y").unwrap();
        std::fs::write(dir.join("types.d.ts"), "z").unwrap();
        std::fs::write(dir.join("node_modules/dep.js"), "n").unwrap();
        let filter = make_filter(
            &["node_modules".to_string()],
            &["AGENTS.md".to_string()],
            &[],
            &[".d.ts".to_string()],
        );
        let hashes = snapshot_hashes(&dir, &filter).unwrap();
        let names: Vec<String> = hashes
            .keys()
            .map(|k| k.rsplit('/').next().unwrap().to_string())
            .collect();
        assert_eq!(names, vec!["keep.ts".to_string()]);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn root_only_exclusion_keeps_nested_namesakes() {
        let dir = std::env::temp_dir().join(format!("napl-snap-root-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("member")).unwrap();
        std::fs::write(dir.join("Cargo.toml"), "root").unwrap();
        std::fs::write(dir.join("member/Cargo.toml"), "member").unwrap();
        std::fs::write(dir.join("member/lib.rs"), "code").unwrap();
        let filter = make_filter(&[], &[], &["Cargo.toml".to_string()], &[]);
        let hashes = snapshot_hashes(&dir, &filter).unwrap();
        let root_manifest = dir.join("Cargo.toml").to_string_lossy().into_owned();
        let member_manifest = dir.join("member/Cargo.toml").to_string_lossy().into_owned();
        let member_lib = dir.join("member/lib.rs").to_string_lossy().into_owned();
        assert!(!hashes.contains_key(&root_manifest));
        assert!(hashes.contains_key(&member_manifest));
        assert!(hashes.contains_key(&member_lib));
        std::fs::remove_dir_all(&dir).ok();
    }
}
