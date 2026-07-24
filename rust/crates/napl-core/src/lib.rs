//! `napl-core`: pure core algorithms for the NAPL toolchain, ported from the
//! TypeScript `@napl/core` and LSP scanner.
//!
//! This crate is phase 1 of an oxc-style Rust rewrite: it contains only the
//! pure, side-effect-free algorithms (diffing, blame, hashing, line math, the
//! prompt scanner, schema validation and extension logic). File I/O, the agent
//! runner, the CLI and the LSP server are out of scope here.

pub mod blame;
pub mod body_lines;
pub mod drift;
pub mod extensions;
pub mod guard;
pub mod hash;
pub mod incremental;
pub mod parse_output;
pub mod prompts;
pub mod scanner;
pub mod schemas;
pub mod targets;
pub mod text_diff;
pub mod yaml;
