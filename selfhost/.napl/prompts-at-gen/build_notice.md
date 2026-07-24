# The deprecated `napl build` command

`napl build` is a deprecated command that no longer generates anything: it prints
a single notice pointing the user at `napl gen` and exits successfully. This
module owns that notice and the command's tiny I/O behavior. It depends on nothing
but the Rust standard library.

## Where this code lives

The working directory is a Cargo workspace whose root manifest is written and
owned by the toolchain — leave it alone. Create this module as its own member
crate in a subdirectory named `build_notice/`: `build_notice/Cargo.toml` (package
name `build_notice`) and `build_notice/src/lib.rs`. Touch nothing outside
`build_notice/`. Ensure `cargo test` passes from the workspace root before
finishing. Add no external dependencies.

## `notice()`

`notice() -> String`: return the deprecation message, **exactly** these bytes on a
single line (an em-dash `—` between "prompts" and "the", straight double quotes
around `napl gen <target>`, a trailing period, no trailing newline):

    napl build is deprecated. Generation now works directly from prompts — the coding agent writes source, and the IR is derived afterwards. Run "napl gen <target>" instead.

Include a `#[cfg(test)]` test asserting `notice()` equals that exact string.

## `run()`

`run() -> i32`: print the `notice()` string followed by a newline to standard
output (a single `println!` of the notice), and return the exit code `0`. This is
the whole behavior of the deprecated command — no filesystem access, no
arguments.
