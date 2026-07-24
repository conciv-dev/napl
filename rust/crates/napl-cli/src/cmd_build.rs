use crate::error::CliResult;

#[allow(clippy::unnecessary_wraps)]
pub fn run() -> CliResult<i32> {
    Ok(build_notice::run())
}
