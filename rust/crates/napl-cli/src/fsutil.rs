pub use fsutil_io::{
    exists, mkdir_parent, read_opt, set_mode, write, EXEC_MODE, READONLY_MODE, WRITABLE_MODE,
};

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
        use std::os::unix::fs::PermissionsExt;
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
