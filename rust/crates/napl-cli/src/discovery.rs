use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::error::{CliError, CliResult};
use crate::paths::rel_to;

pub use discovery_core::declared_crate;

fn read_prompt_files(root: &Path, prompt_files: &[PathBuf]) -> Vec<(String, String)> {
    let mut files = Vec::new();
    for file in prompt_files {
        let Ok(raw) = std::fs::read_to_string(file) else {
            continue;
        };
        files.push((raw, rel_to(root, file)));
    }
    files
}

pub fn check_duplicate_modules(root: &Path, prompt_files: &[PathBuf]) -> CliResult<()> {
    let files = read_prompt_files(root, prompt_files);
    match discovery_core::find_duplicate_module(&files) {
        Some(message) => Err(CliError::new(message)),
        None => Ok(()),
    }
}

pub fn module_paths(root: &Path, prompt_files: &[PathBuf]) -> BTreeMap<String, String> {
    let files = read_prompt_files(root, prompt_files);
    discovery_core::module_paths_from(&files)
}
