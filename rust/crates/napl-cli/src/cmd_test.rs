//! `napl test [target]`: run a target's test command without regenerating.

use std::path::Path;

use napl_core::targets::get_adapter;

use crate::error::{CliError, CliResult};
use crate::paths::resolve_paths;
use crate::process::run_command;

/// Run the target test command and pass through its output and exit code.
pub fn run(root: &Path, target: &str) -> CliResult<i32> {
    let adapter = get_adapter(target).map_err(CliError::new)?;
    let paths = resolve_paths(root);
    let target_dir = paths.src_dir.join(target);
    std::fs::create_dir_all(&target_dir)?;

    let cmd = adapter.test_command(&target_dir.to_string_lossy());
    println!(
        "running {} {} (in {})",
        cmd.command,
        cmd.args.join(" "),
        target_dir.to_string_lossy()
    );
    let result = run_command(&cmd.command, &cmd.args, &target_dir);
    if !result.output.trim().is_empty() {
        println!("{}", result.output.trim_end());
    }
    Ok(result.code)
}
