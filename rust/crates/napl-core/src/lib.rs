//! `napl-core`: pure core algorithms for the NAPL toolchain, ported from the
//! TypeScript `@napl/core` and LSP scanner.
//!
//! This crate is phase 1 of an oxc-style Rust rewrite: it contains only the
//! pure, side-effect-free algorithms (diffing, blame, hashing, line math, the
//! prompt scanner, schema validation and extension logic). File I/O, the agent
//! runner, the CLI and the LSP server are out of scope here.

pub mod blame;
pub mod body_lines;
pub mod extensions;
pub mod hash;
pub mod scanner;
pub mod schemas;
pub mod text_diff;
