//! Path resolution and prompt-file discovery (the I/O counterparts of
//! `paths.ts`).

use std::path::{Path, PathBuf};

use napl_core::extensions::is_prompt_file;

/// The resolved `.napl` layout for a project root.
pub struct NaplPaths {
    /// `.napl/ir`.
    pub ir_dir: PathBuf,
    /// `.napl/src`.
    pub src_dir: PathBuf,
    /// `.napl/map.json`.
    pub map_path: PathBuf,
    /// `.napl/lock.json`.
    pub lock_path: PathBuf,
    /// `.napl/gen.lock`.
    pub gen_lock_path: PathBuf,
    /// `.napl/journal.jsonl`.
    pub journal_path: PathBuf,
    /// `.napl/prompts-at-gen`.
    pub prompts_at_gen_dir: PathBuf,
    /// `examples`.
    pub examples_dir: PathBuf,
    /// `.napl/attribution`.
    pub attribution_dir: PathBuf,
    /// `.napl/mapl`.
    pub ml_dir: PathBuf,
}

/// Resolve the `.napl` layout under `root`, mirroring `resolvePaths`.
#[must_use]
pub fn resolve_paths(root: &Path) -> NaplPaths {
    let napl_dir = root.join(".napl");
    NaplPaths {
        ir_dir: napl_dir.join("ir"),
        src_dir: napl_dir.join("src"),
        map_path: napl_dir.join("map.json"),
        lock_path: napl_dir.join("lock.json"),
        gen_lock_path: napl_dir.join("gen.lock"),
        journal_path: napl_dir.join("journal.jsonl"),
        prompts_at_gen_dir: napl_dir.join("prompts-at-gen"),
        examples_dir: root.join("examples"),
        attribution_dir: napl_dir.join("attribution"),
        ml_dir: napl_dir.join("mapl"),
    }
}

const IGNORED_DIRS: [&str; 3] = ["node_modules", ".napl", ".git"];

fn walk(dir: &Path, aliases: &[String], results: &mut Vec<PathBuf>) -> std::io::Result<()> {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    for entry in entries {
        let entry = entry?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        let full = entry.path();
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            if IGNORED_DIRS.contains(&name.as_ref()) {
                continue;
            }
            walk(&full, aliases, results)?;
        } else if file_type.is_file() && is_prompt_file(&name, Some(aliases)) {
            results.push(full);
        }
    }
    Ok(())
}

/// Discover every prompt file under `root`, sorted, mirroring `findPromptFiles`.
pub fn find_prompt_files(root: &Path, aliases: &[String]) -> std::io::Result<Vec<PathBuf>> {
    let mut results = Vec::new();
    walk(root, aliases, &mut results)?;
    results.sort();
    Ok(results)
}

/// The POSIX-style path of `path` relative to `root`.
#[must_use]
pub fn rel_to(root: &Path, path: &Path) -> String {
    let rel = path.strip_prefix(root).unwrap_or(path);
    rel.to_string_lossy()
        .replace(std::path::MAIN_SEPARATOR, "/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rel_to_produces_posix_paths() {
        let root = Path::new("/project");
        assert_eq!(
            rel_to(root, Path::new("/project/.napl/map.json")),
            ".napl/map.json"
        );
    }

    #[test]
    fn find_prompt_files_ignores_state_dirs_and_sorts() {
        let dir = std::env::temp_dir().join(format!("napl-find-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("examples")).unwrap();
        std::fs::create_dir_all(dir.join(".napl/src")).unwrap();
        std::fs::create_dir_all(dir.join("node_modules")).unwrap();
        std::fs::write(dir.join("examples/b.napl"), "x").unwrap();
        std::fs::write(dir.join("examples/a.napl"), "x").unwrap();
        std::fs::write(dir.join(".napl/src/ignored.napl"), "x").unwrap();
        std::fs::write(dir.join("node_modules/dep.napl"), "x").unwrap();
        std::fs::write(dir.join("examples/notprompt.txt"), "x").unwrap();
        let aliases = napl_core::extensions::default_prompt_aliases();
        let found = find_prompt_files(&dir, &aliases).unwrap();
        let rels: Vec<String> = found.iter().map(|p| rel_to(&dir, p)).collect();
        assert_eq!(
            rels,
            vec!["examples/a.napl".to_string(), "examples/b.napl".to_string()]
        );
        std::fs::remove_dir_all(&dir).ok();
    }
}
