//! Small filesystem helpers used across commands.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

/// Read-only file mode for locked source (`0o444`).
pub const READONLY_MODE: u32 = 0o444;
/// Writable file mode for unlocked source (`0o644`).
pub const WRITABLE_MODE: u32 = 0o644;
/// Executable mode for the installed pre-commit hook (`0o755`).
pub const EXEC_MODE: u32 = 0o755;

/// Read a file to a string, returning `None` when it does not exist.
pub fn read_opt(path: &Path) -> std::io::Result<Option<String>> {
    match fs::read_to_string(path) {
        Ok(content) => Ok(Some(content)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

/// Recursively create the parent directory of `path`.
pub fn mkdir_parent(path: &Path) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)?;
        }
    }
    Ok(())
}

/// Write `content` to `path`, creating parent directories.
pub fn write(path: &Path, content: &str) -> std::io::Result<()> {
    mkdir_parent(path)?;
    fs::write(path, content)
}

/// Set the unix mode bits of `path`.
pub fn set_mode(path: &Path, mode: u32) -> std::io::Result<()> {
    fs::set_permissions(path, fs::Permissions::from_mode(mode))
}

/// Whether a path exists.
pub fn exists(path: &Path) -> bool {
    path.exists()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_opt_returns_none_for_missing() {
        let missing = std::env::temp_dir().join(format!("napl-missing-{}", std::process::id()));
        assert!(read_opt(&missing).unwrap().is_none());
    }

    #[test]
    fn write_then_set_mode_round_trips() {
        let path = std::env::temp_dir().join(format!("napl-mode-{}.txt", std::process::id()));
        write(&path, "hello").unwrap();
        assert_eq!(read_opt(&path).unwrap().as_deref(), Some("hello"));
        set_mode(&path, READONLY_MODE).unwrap();
        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, READONLY_MODE);
        set_mode(&path, WRITABLE_MODE).unwrap();
        std::fs::remove_file(&path).ok();
    }
}
