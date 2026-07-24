//! `napl build`: deprecated, prints a notice pointing at `gen`.

use crate::error::CliResult;

/// Run the deprecated build command.
#[allow(clippy::unnecessary_wraps)]
pub fn run() -> CliResult<i32> {
    println!(
        "napl build is deprecated. Generation now works directly from prompts — the coding agent writes source, and the IR is derived afterwards. Run \"napl gen <target>\" instead."
    );
    Ok(0)
}
