# The snapshot exclusion filter

This module is the **pure** exclusion-predicate core of the CLI's snapshot module:
it decides which directories and files the snapshot walk skips. It is pure — no
filesystem access, no dependencies on other project modules. The I/O shell walks
the tree and consults this filter at each entry; the filter itself only answers
yes/no about a name.

## Where this code lives

The working directory is a Cargo workspace whose root manifest is written and
owned by the toolchain — leave it alone. Create this module as its own member
crate in a subdirectory named `snapshot_filter/`: `snapshot_filter/Cargo.toml`
(package name `snapshot_filter`) and `snapshot_filter/src/lib.rs`. Touch nothing
outside `snapshot_filter/`. Ensure `cargo test` passes from the workspace root
before finishing.

## The filter type

Expose a public struct `SnapshotFilter` that holds four sets of exclusion rules:

- excluded **directory names** — a directory whose file name matches one of these
  is skipped entirely (its subtree is not walked).
- excluded **file names** — a file whose name matches one of these is skipped
  anywhere in the tree.
- excluded **root-only file names** — a file whose name matches one of these is
  skipped **only** when it sits at the walked tree's root, not when a same-named
  file appears in a subdirectory. This exists so a toolchain-owned root manifest
  can be excluded while its per-module namesakes stay included.
- excluded **suffixes** — a file whose name ends with one of these strings is
  skipped anywhere in the tree.

The internal representation is your choice (e.g. hash sets for the exact-name
lists, a vector for the suffixes); only the observable decisions below are pinned.

## `make_filter(exclude_dirs, exclude_files, exclude_root_files, exclude_suffixes)`

`make_filter(exclude_dirs: &[String], exclude_files: &[String], exclude_root_files: &[String], exclude_suffixes: &[String]) -> SnapshotFilter`:
build a `SnapshotFilter` from the four rule lists, in that exact parameter order.
It copies the given entries into the filter; it does no I/O.

## `SnapshotFilter::is_excluded_dir(&self, name: &str) -> bool`

Return `true` when `name` exactly matches one of the excluded directory names,
`false` otherwise. Only exact whole-name matches count — there is no suffix or
prefix logic for directories.

## `SnapshotFilter::is_excluded_file(&self, name: &str, at_root: bool) -> bool`

Return `true` when **any** of these hold, `false` only when none do:

- `name` exactly matches one of the excluded file names, or
- `at_root` is `true` **and** `name` exactly matches one of the excluded
  root-only file names, or
- `name` ends with one of the excluded suffixes.

When `at_root` is `false`, the root-only list is ignored entirely. The plain
file-name and suffix rules apply regardless of `at_root`.

### Worked decisions

With `exclude_dirs = ["node_modules", ".git"]`, `exclude_files = ["AGENTS.md"]`,
`exclude_root_files = ["Cargo.toml"]`, `exclude_suffixes = [".d.ts"]`:

- `is_excluded_dir("node_modules")` → `true`; `is_excluded_dir(".git")` → `true`;
  `is_excluded_dir("src")` → `false`.
- `is_excluded_file("AGENTS.md", false)` → `true` (plain file rule, depth
  irrelevant).
- `is_excluded_file("types.d.ts", false)` → `true` (suffix `.d.ts`).
- `is_excluded_file("Cargo.toml", true)` → `true` (root-only rule at the root).
- `is_excluded_file("Cargo.toml", false)` → `false` (root-only rule does not
  apply below the root, and no plain/suffix rule matches).
- `is_excluded_file("keep.ts", false)` → `false` (no rule matches).
