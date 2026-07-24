//! `napl status`: classify every prompt and exit 1 on drift/unattributed.

use std::path::Path;

use crate::error::CliResult;
use crate::paths::{find_prompt_files, rel_to, resolve_paths};
use crate::state::{load_prompt_aliases, read_journal, read_map, write_map};
use crate::statusclass::classify_prompt;

/// Run status classification across all prompts.
pub fn run(root: &Path) -> CliResult<i32> {
    let paths = resolve_paths(root);
    let mut map = read_map(&paths.map_path)?;
    let (journal, _) = read_journal(&paths.journal_path)?;
    let heals = crate::healing::heal_moved_files(root, &paths, &mut map, &journal)?;
    if !heals.is_empty() {
        write_map(&paths.map_path, &map)?;
    }
    let aliases = load_prompt_aliases(&paths.lock_path);
    let prompt_files = find_prompt_files(root, &aliases)?;
    crate::discovery::check_duplicate_modules(root, &prompt_files)?;

    let mut any_error = false;
    for file in prompt_files {
        let rel = rel_to(root, &file);
        let raw = std::fs::read_to_string(&file)?;
        let entry = classify_prompt(root, &rel, &raw, &map)?;
        if entry.is_error() {
            any_error = true;
        }
        println!("{}", entry.line());
    }

    Ok(i32::from(any_error))
}
