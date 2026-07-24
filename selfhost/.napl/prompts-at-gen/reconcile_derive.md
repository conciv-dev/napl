# Deriving reconcile inputs from drifted files

This module is the **pure** derivation core of the CLI's reconcile command: given
the drifted files a module's drift detection already produced, it decides which
files can be folded back into the prompt and builds the per-file inputs the
reconcile task needs. It is pure — no filesystem access, no process spawning. The
I/O shell detects drift, calls these functions over the already-read drift data,
then runs the coding agent and journals the result.

## Where this code lives

The working directory is a Cargo workspace whose root manifest is written and
owned by the toolchain — leave it alone. Create this module as its own member
crate in a subdirectory named `reconcile_derive/`: `reconcile_derive/Cargo.toml`
(package name `reconcile_derive`) and `reconcile_derive/src/lib.rs`. Touch nothing
outside `reconcile_derive/`. Ensure `cargo test` passes from the workspace root
before finishing.

## Builds on the `drift`, `prompts`, and `text_diff` modules of this workspace

This module composes on three **generated sibling member crates** in the same
workspace — not on crates.io dependencies and not on any hand-written crate. Add
path dependencies on them in your `Cargo.toml`:

- `drift` lives at `../drift`. Use its public `DriftedFile` struct and its public
  `DriftReason` enum. `DriftReason` has variants `Edited` (a file changed by hand)
  and `Missing` (a locked file deleted). `DriftedFile` has fields including `file:
  String` (path relative to the project root), `reason: DriftReason`, `current:
  Option<String>` (the current on-disk content if present), and `diff:
  Option<String>` (the baseline-to-current unified diff if one was computed).
- `prompts` lives at `../prompts`. Use its public `ReconcileFile` struct, which
  has public fields `file: String` and `diff: String`. Do not reimplement it.
- `text_diff` lives at `../text_diff`. Use its public `unified_diff(before: &str,
  after: &str) -> String` function to build a diff. Do not reimplement it.

Depending on a sibling by path only reads it; leave all three sibling crates
untouched.

## `editable_drifted(files: &[DriftedFile]) -> Vec<&DriftedFile>`

Return references to the drifted files that a reconcile can fold back into the
prompt: keep exactly those whose `reason` is `Edited` **and** whose `current`
content is `Some`, preserving the original order of `files`. Drop every `Missing`
file (it was deleted, so there is nothing to accept as a new baseline) and every
`Edited` file whose `current` is `None` (its content is not available). For a
list `[a.ts (edited, current), b.ts (missing), c.ts (edited, no current), d.ts
(edited, current)]`, the editable files are `[a.ts, d.ts]` in that order.

## `build_reconcile_files(files: &[DriftedFile]) -> Vec<ReconcileFile>`

Build one `ReconcileFile` for each editable file (using the same editability rule
as `editable_drifted`, in the same order). For each editable file:

- `file` is the drifted file's `file`.
- `diff` is the drifted file's recorded `diff` when it is `Some` (used verbatim);
  otherwise it is `unified_diff("", current)` where `current` is the file's
  current content (treat a `None` current as the empty string — though an editable
  file always has `Some` current).

So an editable file carrying a recorded diff `"PRERECORDED DIFF"` yields a
`ReconcileFile { file, diff: "PRERECORDED DIFF" }`, and an editable file with no
recorded diff and current content `"new content"` yields a `ReconcileFile` whose
`diff` equals `unified_diff("", "new content")`. Deleted or content-less files
contribute no `ReconcileFile`, so a list containing only a `Missing` file and an
`Edited` file with no current content yields an empty vector.
