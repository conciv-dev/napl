//! Small filesystem primitives for the CLI.
//!
//! The thin filesystem seam shared across the CLI's commands: read a file that
//! may be absent, create parent directories, write a file, set unix mode bits,
//! and test existence. This is an I/O shell — every function touches the real
//! filesystem — so it declares no given/expect corpus; its behavior is pinned
//! by filesystem round-trip tests. It depends on nothing but the standard
//! library.

use std::fs;
use std::io;
use std::path::Path;

/// The read-only mode for locked, generated source.
pub const READONLY_MODE: u32 = 0o444;
/// The writable mode for unlocked source.
pub const WRITABLE_MODE: u32 = 0o644;
/// The executable mode for the installed pre-commit hook.
pub const EXEC_MODE: u32 = 0o755;

/// Read the whole file at `path` to a `String`.
///
/// Returns `Ok(Some(content))` when it is read, and `Ok(None)` **only** when
/// the file does not exist. Every other I/O error is propagated as `Err`, so a
/// permission failure never looks like absence.
pub fn read_opt(path: &Path) -> io::Result<Option<String>> {
    match fs::read_to_string(path) {
        Ok(content) => Ok(Some(content)),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(err),
    }
}

/// Recursively create the parent directory of `path` (like `mkdir -p` on the
/// parent).
///
/// If `path` has a non-empty parent component, it and all missing ancestors are
/// created; if there is no parent, or the parent is the empty path, this does
/// nothing. Creating a tree that already exists is not an error.
pub fn mkdir_parent(path: &Path) -> io::Result<()> {
    match path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => fs::create_dir_all(parent),
        _ => Ok(()),
    }
}

/// Create the parent directory of `path`, then write `content` to `path`,
/// truncating any existing file.
pub fn write(path: &Path, content: &str) -> io::Result<()> {
    mkdir_parent(path)?;
    fs::write(path, content)
}

/// Set the unix permission bits of `path` to `mode`.
pub fn set_mode(path: &Path, mode: u32) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(mode))
}

/// Return whether `path` exists on disk.
pub fn exists(path: &Path) -> bool {
    path.exists()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;
    use std::process;

    /// A unique temporary directory for one test, removed on drop.
    struct TmpDir {
        path: PathBuf,
    }

    impl TmpDir {
        fn new(tag: &str) -> Self {
            let mut path = std::env::temp_dir();
            // pid + a per-tag suffix keeps concurrent tests from colliding.
            path.push(format!("fsutil_io-{}-{}", process::id(), tag));
            fs::create_dir_all(&path).unwrap();
            TmpDir { path }
        }

        fn join(&self, name: &str) -> PathBuf {
            self.path.join(name)
        }
    }

    impl Drop for TmpDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn read_opt_missing_is_none() {
        let tmp = TmpDir::new("missing");
        let path = tmp.join("does-not-exist.txt");
        assert_eq!(read_opt(&path).unwrap(), None);
    }

    #[test]
    fn write_then_read_round_trips() {
        let tmp = TmpDir::new("roundtrip");
        let path = tmp.join("nested/dir/file.txt");

        write(&path, "hello").unwrap();
        assert_eq!(read_opt(&path).unwrap(), Some("hello".to_string()));

        set_mode(&path, READONLY_MODE).unwrap();
        let mode = fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o444);

        set_mode(&path, WRITABLE_MODE).unwrap();
        let mode = fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o644);
    }

    #[test]
    fn write_creates_missing_parents() {
        let tmp = TmpDir::new("parents");
        let path = tmp.join("a/b/c/deep.txt");
        write(&path, "x").unwrap();
        assert!(exists(&path));
    }

    #[test]
    fn mkdir_parent_without_parent_is_ok() {
        // A bare relative name has an empty parent — must be a no-op, not an error.
        mkdir_parent(Path::new("bare-name")).unwrap();
    }

    #[test]
    fn exists_reflects_disk() {
        let tmp = TmpDir::new("exists");
        let path = tmp.join("f.txt");
        assert!(!exists(&path));
        write(&path, "").unwrap();
        assert!(exists(&path));
    }
}
