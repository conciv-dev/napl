# Rendering `napl blame` output

This module is the **pure** rendering core of the CLI's blame command: given
journal and blame data that the I/O shell has already read from disk, it produces
the exact text the command prints. It is pure — no filesystem access, no process
spawning. The shell reads the journal, computes blame, then calls these functions
and prints their strings.

## Where this code lives

The working directory is a Cargo workspace whose root manifest is written and
owned by the toolchain — leave it alone. Create this module as its own member
crate in a subdirectory named `blame_render/`: `blame_render/Cargo.toml`
(package name `blame_render`) and `blame_render/src/lib.rs`. Touch nothing
outside `blame_render/`. Ensure `cargo test` passes from the workspace root
before finishing.

## Builds on the `blame` and `schemas_journal` modules of this workspace

This module composes on two **generated sibling member crates** in the same
workspace — not on crates.io dependencies and not on any hand-written crate. Add
path dependencies on them in your `Cargo.toml`:

- `blame` lives at `../blame`. Use its public `BlameLine` struct (fields `line:
  usize`, `gen: i64`, `timestamp: String`, `module: String`, `text: String`) and
  its public `first_prompt_diff_line(prompt_diff: &str) -> String` function. Do
  not reimplement either.
- `schemas_journal` lives at `../schemas_journal`. Use its public `JournalEntry`
  struct (fields include `gen: i64`, `timestamp: String`, `module: String`,
  `target: String`, `prompt_diff: String`, `mode: JournalMode`, and `files:
  Vec<JournalFile>` where each `JournalFile` has a `path: String` field) and its
  public `JournalMode` enum (variants `Full`, `Incremental`, `Reconcile`). Do not
  reimplement them.

Depending on a sibling by path only reads it; leave both sibling crates
untouched.

## `mode_str(mode: JournalMode) -> &'static str`

Map a journal mode to its lowercase label: `Full` → `"full"`, `Incremental` →
`"incremental"`, `Reconcile` → `"reconcile"`.

## `format_blame_row(entry: &BlameLine) -> String`

Render one blame row as exactly `"gen #{gen}  {timestamp}  {module}  {text}"`,
where each `{…}` is the corresponding `BlameLine` field and the separators are
**two spaces**. For a line with `gen = 7`, `timestamp =
"2026-07-24T00:00:00.000Z"`, `module = "greeting"`, `text = "export function
greet() {"`, the row is
`gen #7  2026-07-24T00:00:00.000Z  greeting  export function greet() {`.

## `why_line(prompt_diff: &str) -> String`

Explain why a line exists. Call `blame::first_prompt_diff_line(prompt_diff)`; if
the result is the empty string, return `"initial generation"`, otherwise return
the result unchanged. So an empty prompt diff yields `"initial generation"`, and a
diff whose first added hunk line is `the new behavior line` yields `the new
behavior line`.

## `render_blame_gen(entries: &[JournalEntry], gen: i64) -> BlameGenRender`

Expose a public struct `BlameGenRender` with public fields `text: String` and
`exit_code: i32`. This function renders the single-generation summary the command
prints for `napl blame --gen <n>`.

Find the first entry in `entries` whose `gen` equals the requested `gen`.

**When no entry matches**, return `BlameGenRender { text: format!("napl blame: no
journal entry for gen #{gen}"), exit_code: 1 }` — the text is exactly
`napl blame: no journal entry for gen #{gen}` with the requested number.

**When an entry matches**, build the text as a sequence of lines joined by single
newlines (`\n`), with **no trailing newline**, and `exit_code: 0`. The lines are,
in order:

1. `gen #{gen}  {timestamp}  {module} ({target})  mode: {mode}` — the entry's
   `gen`, `timestamp`, `module`, `target`, and `mode_str(entry.mode)`, with two
   spaces between the segments and the target in parentheses.
2. an empty line.
3. `prompt edit:`.
4. the prompt-diff block: if `entry.prompt_diff` is empty **after trimming
   surrounding whitespace**, a single line `  initial generation` (two leading
   spaces); otherwise, split `entry.prompt_diff` on `\n` and emit one line per
   piece, each prefixed with two spaces (so a trailing newline in the diff yields a
   final `  ` two-space line).
5. an empty line.
6. `files touched:`.
7. the files block: if `entry.files` is empty, a single line `  (none)` (two
   leading spaces); otherwise one line per file, each `  {path}` (two leading
   spaces then the file's `path`).

For an entry with `gen = 1`, `timestamp = "2026-07-24T00:00:00.000Z"`, `module =
"greeting"`, `target = "typescript"`, `mode = Full`, an empty `prompt_diff`, and
no files, the text is exactly:

`gen #1  2026-07-24T00:00:00.000Z  greeting (typescript)  mode: full\n\nprompt edit:\n  initial generation\n\nfiles touched:\n  (none)`

The shell prints this text with a single trailing newline and exits with
`exit_code`.
