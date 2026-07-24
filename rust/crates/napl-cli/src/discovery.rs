//! Location-free prompt identity: a prompt is identified by its frontmatter
//! `module` name, never by its path on disk. This module walks the discovered
//! prompt files, resolves each to its declared module, and enforces the
//! cp-semantics guard — two prompts that declare the same module are a hard
//! error, because moving or copying a prompt file must not silently fork or
//! shadow an identity.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use napl_core::schemas::parse_frontmatter;

use crate::error::{CliError, CliResult};
use crate::paths::rel_to;

/// Reject two prompts that declare the same module name. Identity is the
/// module, so a copied prompt must be renamed in the copy. Prompts whose
/// frontmatter fails to parse are skipped here and left for the caller's
/// per-file classification to surface the parse error in its own order.
pub fn check_duplicate_modules(root: &Path, prompt_files: &[PathBuf]) -> CliResult<()> {
    let mut seen: BTreeMap<String, String> = BTreeMap::new();
    for file in prompt_files {
        let Ok(raw) = std::fs::read_to_string(file) else {
            continue;
        };
        let Ok(parsed) = parse_frontmatter(&raw) else {
            continue;
        };
        let module = parsed.frontmatter.module;
        let rel = rel_to(root, file);
        if let Some(existing) = seen.get(&module) {
            return Err(CliError::new(format!(
                "duplicate module '{module}' is declared by two prompts: {existing} and {rel}. Copied a prompt? Rename the module in the copy."
            )));
        }
        seen.insert(module, rel);
    }
    Ok(())
}

/// The optional `crate:` frontmatter key: the declared member crate a module's
/// generated code groups into. Absent means the module owns a crate named after
/// itself (the default layout). `crate` is now a strict, known field of the
/// generated `Frontmatter` schema (deserialized into `crate_name`), so this reads
/// it straight off the strict parse instead of re-scanning the raw YAML. A prompt
/// whose frontmatter fails to parse yields `None` (its parse error is surfaced by
/// the caller's per-file classification).
pub fn declared_crate(raw: &str) -> Option<String> {
    parse_frontmatter(raw).ok()?.frontmatter.crate_name
}

/// Build the derived `module -> current on-disk relative path` index from the
/// discovered prompt files. This is how a prompt's location is resolved now that
/// the map keys identity by module and stores no path: the path is always the
/// one found on disk this run. Prompts whose frontmatter fails to parse are
/// skipped.
pub fn module_paths(root: &Path, prompt_files: &[PathBuf]) -> BTreeMap<String, String> {
    let mut paths = BTreeMap::new();
    for file in prompt_files {
        let Ok(raw) = std::fs::read_to_string(file) else {
            continue;
        };
        let Ok(parsed) = parse_frontmatter(&raw) else {
            continue;
        };
        paths.insert(parsed.frontmatter.module, rel_to(root, file));
    }
    paths
}
