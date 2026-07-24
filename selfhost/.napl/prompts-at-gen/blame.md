# Patch-replay line ancestry ("blame")

This module computes **line-level generation ancestry** for a generated file: for
each line of the current content, which generation last wrote it, plus that
generation's timestamp and module. It replays the sequence of unified-diff
patches recorded across generations against a per-line generation vector. It is
pure: no I/O.

## Where this code lives

The working directory is a Cargo workspace whose root manifest is written and
owned by the toolchain — leave it alone. Create this module as its own member
crate in a subdirectory named `blame/`: `blame/Cargo.toml` (package name `blame`)
and `blame/src/lib.rs`. Touch nothing outside `blame/`. Ensure `cargo test`
passes from the workspace root before finishing.

## Builds on the `text_diff` module of this workspace

Patches are unified diffs. Parsing a patch into hunks, splitting content into
lines, and (in your tests) producing a diff between two texts are all the
**`text_diff`** module of this same workspace, generated as a sibling member
crate in the `text_diff/` directory (so `../text_diff` relative to this crate).
It exposes:

- `to_lines(text: &str) -> Vec<String>`: split text into its lines (empty text →
  no lines; a single trailing `\n`/`\r\n` terminator removed).
- `parse_hunks(diff: &str) -> Vec<Hunk>`: parse a unified diff into hunks. A
  `Hunk` has public `old_start: usize` (the 1-based old-file start line),
  `old_count`, `new_start`, `new_count`, and `lines: Vec<HunkLine>`. A `HunkLine`
  has public `kind: HunkKind` and `text: String`, where `HunkKind` is one of
  `Context`, `Del`, `Ins`.
- `unified_diff(before: &str, after: &str) -> String`: the unified diff between
  two texts (use this in your test helpers to build patches from file contents).

Add a path dependency on that sibling member crate in your `Cargo.toml` (depend
on the `text_diff` crate by path — it lives at `../text_diff`) and use its hunk
parsing, line splitting, and diffing. Do **not** reimplement diff parsing or line
splitting here, and do not depend on any hand-written crate — depend only on the
generated sibling `text_diff` member crate. This sibling path dependency is part
of the same workspace, not an external crates.io dependency, so add it even
though the general guidance is to avoid outside dependencies.

## The types

Expose these public types (comparable, cloneable):

- `BlameSourceEntry`: one recorded generation touching a file, with public
  fields `gen: i64`, `timestamp: String`, `module: String`, `patch: String`
  (the unified-diff patch that generation applied to the file), and
  `prompt_diff: String` (the diff of the prompt body at that generation).
- `BlameLine`: one blamed line of the current content, with public fields
  `line: usize` (1-based), `gen: i64`, `timestamp: String`, `module: String`,
  and `text: String`.

## Replaying a patch onto a blame vector

`apply_patch_to_blame(blame: &[i64], patch: &str, gen: i64) -> Vec<i64>` replays
one patch against a per-line generation vector (`blame[i]` is the generation that
wrote old line *i*), returning the new vector for the post-patch content, tagging
inserted lines with `gen`:

- Parse the patch into hunks. If there are none, return the input vector
  unchanged.
- Walk a cursor `old_idx` over the old lines starting at 0. For each hunk, first
  copy untouched old entries up to (but not including) the hunk's start —
  i.e. while `old_idx < hunk.old_start - 1` (saturating at 0) and within bounds,
  push `blame[old_idx]` and advance. Then process the hunk's lines in order:
  - a **context** line pushes `blame[old_idx]` (or `gen` if past the end) and
    advances `old_idx`;
  - a **deletion** line advances `old_idx` without pushing;
  - an **insertion** line pushes `gen` and does not advance `old_idx`.
- After the last hunk, push any remaining `blame[old_idx..]` entries in order.

## Blaming a whole file

`blame_file(history: &[BlameSourceEntry], current_content: &str) -> Vec<BlameLine>`:

- Sort a copy of `history` ascending by `gen`.
- Fold the patches: start with an empty blame vector and, for each entry in
  ascending order, set `blame = apply_patch_to_blame(&blame, &entry.patch, entry.gen)`.
- Build a lookup from `gen` to its entry. Choose a fallback generation: the last
  (highest-gen) entry's `gen`, or `0` if history is empty.
- For each line of `to_lines(current_content)` at 0-based `index`: its generation
  is `blame[index]` if present, else the fallback. Produce a `BlameLine` with
  `line = index + 1`, that `gen`, the `timestamp` and `module` of the looked-up
  entry (empty strings if none), and `text` = the line.

`blame_line_at(history, current_content, line) -> Option<BlameLine>`: the single
`BlameLine` whose `line` equals the 1-based `line`, or `None` if out of range.

## The most informative prompt-diff line

`first_prompt_diff_line(prompt_diff: &str) -> String`: scan the diff's lines
(stripping a trailing `\r` from each). Ignore everything until the first hunk
header (a line starting with `@@`); skip header lines themselves. For each
remaining line, take its text as everything after the first character (the
`+`/`-`/space marker), trimmed; skip lines whose trimmed text is empty. Return the
first added line's text (a line starting with `+`) if any; otherwise the first
removed line's text (a line starting with `-`); otherwise the empty string.
