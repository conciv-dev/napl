//! The deprecated `napl build` command.
//!
//! `napl build` no longer generates anything: it prints a single notice
//! pointing the user at `napl gen` and exits successfully.

/// The deprecation message for `napl build`, on a single line with no trailing
/// newline.
pub fn notice() -> String {
    String::from(
        "napl build is deprecated. Generation now works directly from prompts — the coding agent writes source, and the IR is derived afterwards. Run \"napl gen <target>\" instead.",
    )
}

/// Run the deprecated `napl build` command: print the notice and return the
/// exit code `0`.
pub fn run() -> i32 {
    println!("{}", notice());
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notice_is_exact() {
        assert_eq!(
            notice(),
            "napl build is deprecated. Generation now works directly from prompts — the coding agent writes source, and the IR is derived afterwards. Run \"napl gen <target>\" instead."
        );
    }

    #[test]
    fn notice_has_no_trailing_newline() {
        assert!(!notice().ends_with('\n'));
    }

    #[test]
    fn run_returns_zero() {
        assert_eq!(run(), 0);
    }
}
