# Small filesystem primitives for the CLI

This module is the thin filesystem seam shared across the CLI's commands: read a
file that may be absent, create parent directories, write a file, set unix mode
bits, and test existence. It is an **I/O shell**, not a pure module — every
function touches the real filesystem — so it declares no given/expect corpus; its
behavior is pinned end-to-end by the conformance suite and by its own filesystem
round-trip tests. It depends on nothing but the Rust standard library.

## Where this code lives

The working directory is a Cargo workspace whose root manifest is written and
owned by the toolchain — leave it alone. Create this module as its own member
crate in a subdirectory named `fsutil_io/`: `fsutil_io/Cargo.toml` (package name
`fsutil_io`) and `fsutil_io/src/lib.rs`. Touch nothing outside `fsutil_io/`.
Ensure `cargo test` passes from the workspace root before finishing. Add no
external dependencies — the standard library is sufficient.

## The mode constants

Three public `u32` constants naming unix permission bit patterns:

- `READONLY_MODE = 0o444` — the read-only mode for locked, generated source.
- `WRITABLE_MODE = 0o644` — the writable mode for unlocked source.
- `EXEC_MODE = 0o755` — the executable mode for the installed pre-commit hook.

## `read_opt(path)`

`read_opt(path: &std::path::Path) -> std::io::Result<Option<String>>`: read the
whole file at `path` to a `String`. Return `Ok(Some(content))` when it is read;
return `Ok(None)` **only** when the file does not exist (the not-found I/O error
kind); propagate every other I/O error as `Err`. A caller uses this to treat an
absent file as an empty/default state without swallowing real errors (a permission
failure must not look like absence).

## `mkdir_parent(path)`

`mkdir_parent(path: &std::path::Path) -> std::io::Result<()>`: recursively create
the parent directory of `path` (like `mkdir -p` on the parent). If `path` has a
parent component that is non-empty, create it and all missing ancestors; if there
is no parent, or the parent is the empty path, do nothing and return `Ok(())`.
Creating a directory tree that already exists is not an error.

## `write(path, content)`

`write(path: &std::path::Path, content: &str) -> std::io::Result<()>`: create the
parent directory of `path` (via the same recursive parent creation as
`mkdir_parent`), then write `content` to `path`, truncating any existing file.

## `set_mode(path, mode)`

`set_mode(path: &std::path::Path, mode: u32) -> std::io::Result<()>`: set the unix
permission bits of `path` to `mode` (a raw mode value such as one of the constants
above). This is unix-specific; use the standard library's unix permissions
extension.

## `exists(path)`

`exists(path: &std::path::Path) -> bool`: return whether `path` exists on disk.

## Filesystem round-trip tests to include

Write the crate's own `#[cfg(test)]` tests against a unique temporary path:

- Reading a path that does not exist returns `Ok(None)`.
- After `write(path, "hello")`, `read_opt(path)` returns `Ok(Some("hello"))`;
  then `set_mode(path, READONLY_MODE)` makes the low nine mode bits equal
  `0o444`, and `set_mode(path, WRITABLE_MODE)` restores `0o644`.
