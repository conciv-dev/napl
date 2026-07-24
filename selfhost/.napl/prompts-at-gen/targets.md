# Target adapters

This module is the per-language target registry the toolchain consults when it
generates, tests, and attributes code. Each target names its coding-agent idiom
guidance, the tools the agent may use, the attribution exclusion rules, and the
test command that gates generation. It is pure data: no I/O and no dependencies
on other project modules.

## Where this code lives

The working directory is a Cargo workspace whose root manifest is written and
owned by the toolchain — leave it alone. Create this module as its own member
crate in a subdirectory named `targets/`: `targets/Cargo.toml` (package name
`targets`) and `targets/src/lib.rs`. Touch nothing outside `targets/`. Ensure
`cargo test` passes from the workspace root before finishing.

## The data types

- `TestRunCommand`, a struct with public fields `command` (a `String`, the
  executable) and `args` (a `Vec<String>`, its arguments). Support
  equality/cloning.
- `TargetAdapter`, a struct with these public fields:
  - `name`: a `&'static str`, the target name (also its source subdirectory).
  - `idiom_guidance`: a `String`, guidance injected into the coding-agent task.
  - `agent_tools`: a `Vec<String>`, the tools the agent may use.
  - `attribution_exclude_dirs`: a `Vec<String>`, directory names excluded from
    attribution snapshots.
  - `attribution_exclude_files`: a `Vec<String>`, file names excluded everywhere.
  - `attribution_exclude_root_files`: a `Vec<String>`, file names excluded only at
    the target-directory root (used for a toolchain-owned root manifest whose
    same-named per-module siblings stay attributed).
  - `attribution_exclude_suffixes`: a `Vec<String>`, file suffixes excluded.
  - `test_command_label`: a `&'static str`, the human-facing test-command label.
  - `test_run`: a `TestRunCommand`, the test invocation gating generation.
  - `workspace_layout`: a `bool` — true when the target dir is a workspace of
    per-module member crates, false for a single package rooted at the target dir.

  Give `TargetAdapter` a method `test_command(&self, target_dir: &str)` returning
  the `TestRunCommand` (the command is fixed per target; the directory argument is
  accepted but not consulted).

## The registry

- `list_targets()` returns the registered target names, in registry order, as
  `["typescript", "react", "rust"]`.
- `starter_targets()` returns the targets whose guard directory `napl init`
  pre-creates: `["typescript", "react"]` (a subset of the registered set — the
  third target, `rust`, is fully usable but not scaffolded up front).
- `get_adapter(name)` looks up an adapter by name, returning it on success or, for
  an unregistered name, an error string of the form
  `unknown target '<name>'. Available targets: typescript, react, rust`. (So an
  unknown target `cobol` yields an error containing `unknown target 'cobol'` and
  `typescript, react, rust`.)

## The three adapters

- **typescript**: name `typescript`; test command label `npx vitest run`, whose
  `TestRunCommand` is command `npx` with args `["vitest", "run"]`; agent tools
  include `Read` (and the usual npm/npx/node/file tools); attribution excludes the
  usual JS dirs (`node_modules`, `dist`, `.git`, `build`, `coverage`, `.vite`) and
  files (`package-lock.json`, `AGENTS.md`, `CLAUDE.md`) and suffixes (`.tsbuildinfo`,
  `.d.ts`); it does **not** exclude `vite.config.js`; `workspace_layout` is false
  and `attribution_exclude_root_files` is empty.
- **react**: like typescript (same test command, tools, and base excludes) but its
  `attribution_exclude_files` additionally includes `vite.config.js`;
  `workspace_layout` is false and `attribution_exclude_root_files` is empty.
- **rust**: name `rust`; test command label `cargo test`, whose `TestRunCommand` is
  command `cargo` with args `["test"]`; agent tools include `Read` and
  `Bash(cargo:*)` (and rustc/rustfmt/file tools) and do **not** include
  `Bash(npx:*)`; attribution excludes dirs including `target` and files including
  `Cargo.lock`; `workspace_layout` is **true** and
  `attribution_exclude_root_files` includes `Cargo.toml` (the toolchain-owned
  workspace root manifest). Its `idiom_guidance` describes a Rust library crate
  tested with `cargo test` and must mention that the working directory is a Cargo
  WORKSPACE (include the words "Cargo WORKSPACE").

## The workspace manifest — `workspace_manifest_toml(members)`

Render the workspace root `Cargo.toml` for a Cargo-workspace target. Given a
slice of member crate directory names (already sorted by the caller), return a
deterministic TOML string containing a `[workspace]` table with `resolver = "2"`
and a `members` array listing each member as a quoted string on its own line. The
same member slice always yields byte-identical output. For members `["body_lines",
"extensions", "hash"]` the output contains `[workspace]`, `resolver = "2"`, and
the lines `"body_lines",`, `"extensions",`, `"hash",`. For an empty member slice
the output contains an empty members array rendered as `members = [` then a line
`]`.
