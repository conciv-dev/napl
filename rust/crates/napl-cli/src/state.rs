//! On-disk state readers/writers: map, journal, lock. The pure parsing and
//! serialization live in `napl-core`; this module adds the file I/O.

use std::fs;
use std::path::Path;

use napl_core::extensions::default_prompt_aliases;
use napl_core::schemas::{
    empty_map, map_to_json, parse_lock, parse_map, read_journal_str, resolve_prompt_aliases,
    Backend, HlLock, JournalEntry, NaplMap,
};

use crate::error::{CliError, CliResult};
use crate::fsutil;

/// Read the map, or an empty map when absent, mirroring `readMap`.
pub fn read_map(map_path: &Path) -> CliResult<NaplMap> {
    match fsutil::read_opt(map_path)? {
        Some(raw) => Ok(parse_map(&raw)?),
        None => Ok(empty_map()),
    }
}

/// Write the map pretty-printed with a trailing newline, mirroring `writeMap`.
pub fn write_map(map_path: &Path, map: &NaplMap) -> CliResult<()> {
    fsutil::write(map_path, &map_to_json(map))?;
    Ok(())
}

/// Read the journal, returning valid entries and skip-warnings, mirroring
/// `readJournal` (the caller decides how to surface warnings).
pub fn read_journal(journal_path: &Path) -> CliResult<(Vec<JournalEntry>, Vec<String>)> {
    match fsutil::read_opt(journal_path)? {
        Some(raw) => Ok(read_journal_str(&raw)),
        None => Ok((Vec::new(), Vec::new())),
    }
}

/// Append one compact JSON journal line, mirroring `appendJournalEntry`.
pub fn append_journal_entry(journal_path: &Path, entry: &JournalEntry) -> CliResult<()> {
    fsutil::mkdir_parent(journal_path)?;
    let line = serde_json::to_string(entry).map_err(|e| CliError::new(e.to_string()))?;
    let mut existing = fsutil::read_opt(journal_path)?.unwrap_or_default();
    existing.push_str(&line);
    existing.push('\n');
    fs::write(journal_path, existing)?;
    Ok(())
}

/// Read and validate the lock, mirroring `readLock`.
pub fn read_lock(lock_path: &Path) -> CliResult<HlLock> {
    match fsutil::read_opt(lock_path)? {
        Some(raw) => Ok(parse_lock(&raw)?),
        None => Err(CliError::new(
            "missing .napl/lock.json — run 'napl init' first",
        )),
    }
}

/// Write the lock pretty-printed with a trailing newline, mirroring `writeLock`.
pub fn write_lock(lock_path: &Path, lock: &HlLock) -> CliResult<()> {
    let mut json = serde_json::to_string_pretty(lock).map_err(|e| CliError::new(e.to_string()))?;
    json.push('\n');
    fsutil::write(lock_path, &json)?;
    Ok(())
}

/// Build the default lock document written by `init`.
#[must_use]
pub fn default_lock() -> HlLock {
    HlLock {
        model: napl_core::schemas::DEFAULT_MODEL.to_string(),
        backend: Backend::ClaudeCli,
        prompt_aliases: None,
    }
}

/// Resolve prompt aliases from the lock, falling back to defaults, mirroring
/// `loadPromptAliases`.
pub fn load_prompt_aliases(lock_path: &Path) -> Vec<String> {
    match fsutil::read_opt(lock_path) {
        Ok(Some(raw)) => match parse_lock(&raw) {
            Ok(lock) => resolve_prompt_aliases(&lock),
            Err(_) => default_prompt_aliases(),
        },
        _ => default_prompt_aliases(),
    }
}
