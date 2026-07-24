# The generation journal: an append-only JSONL history

This module defines the serde types and reader for the NAPL **generation
journal** — the append-only JSONL log that records one entry per generation of a
module against a target, and the small helpers that read and query it. It is
pure: no I/O (the caller reads the file; this module parses the text). Bring in
`serde`, `serde_json`.

## Where this code lives

The working directory is a Cargo workspace whose root manifest is written and
owned by the toolchain — leave it alone. Create this module as its own member
crate in a subdirectory named `schemas_journal/`: `schemas_journal/Cargo.toml`
(package name `schemas_journal`) and `schemas_journal/src/lib.rs`. Touch nothing
outside `schemas_journal/`. Ensure `cargo test` passes from the workspace root
before finishing.

## Builds on two modules of this workspace

Both are sibling member crates generated into this same workspace. Add a path
dependency on each in your `Cargo.toml` (they are workspace siblings, not
external crates.io dependencies, so add them even though the general guidance is
to avoid outside dependencies), use their public API, and do **not** reimplement
their logic or depend on any hand-written crate — depend only on the generated
siblings.

- **`text_diff`** (`../text_diff`): exposes
  `unified_diff(before: &str, after: &str) -> String`, the unified diff between
  two texts. This module's `file_patch` is exactly this diff.
- **`blame`** (`../blame`): exposes a public struct `BlameSourceEntry` with
  public fields `gen: i64`, `timestamp: String`, `module: String`,
  `patch: String`, and `prompt_diff: String`. This module's `file_history`
  returns a `Vec<BlameSourceEntry>`.

## The types

Expose these public serde types (each `Debug`, `Clone`, `PartialEq`, `Eq`, and
both `Serialize` and `Deserialize`):

- `JournalFile`: one file touched by a generation. Public fields:
  - `path: String`
  - `patch: String`
  - `hash_before: Option<String>` — serde field name `hashBefore`.
  - `hash_after: String` — serde field name `hashAfter`.
- `JournalMode`: an enum (`Copy` as well) with variants `Full`, `Incremental`,
  `Reconcile`, `Move`, serialized in lowercase (`full`, `incremental`,
  `reconcile`, `move`). The `Move` mode records a location-heal: a generated file
  the toolchain relocated to a new path when its content was found unchanged (or
  drifted) under a different filename, with the file entry's patch describing the
  move.
- `JournalEntry`: one generation. Public fields:
  - `gen: i64`
  - `timestamp: String`
  - `module: String`
  - `target: String`
  - `prompt_hash: String` — serde field name `promptHash`.
  - `prompt_diff: String` — serde field name `promptDiff`.
  - `mode: JournalMode`
  - `files: Vec<JournalFile>` — defaulting to empty when the JSON omits it.

## Building a file patch

`file_patch(before: Option<&str>, after: &str) -> String`: the unified diff for a
created file (`before` is `None`, treated as the empty string) or a modified
file. Delegate to `text_diff::unified_diff`, passing `""` when `before` is `None`.

## Reading the journal, skipping corrupt lines

`read_journal_str(raw: &str) -> (Vec<JournalEntry>, Vec<String>)` parses the raw
JSONL text and returns the valid entries plus one warning string per skipped
line. Walk the lines by splitting on `\n`, stripping a trailing `\r` from each,
tracking a 1-based line number. For each line:

- If the line is empty after trimming, skip it silently (no entry, no warning).
- First try to parse the line as **arbitrary JSON** (a generic JSON value). If it
  is **not syntactically valid JSON**, push a warning and move on (no entry). This
  warning text is **byte-exact and load-bearing** — a downstream command prints it
  verbatim to the user, so it must match to the byte:

      journal: skipping corrupt line {n} (invalid JSON)

  where `{n}` is the 1-based line number. For example, a non-JSON second line
  yields exactly `journal: skipping corrupt line 2 (invalid JSON)` — the literal
  prefix `journal: skipping corrupt line `, the number, then ` (invalid JSON)`.
- Otherwise the line **is** syntactically valid JSON: deserialize it as a
  `JournalEntry` and run validation (below). If either the deserialize or the
  validation fails, push a warning of the form
  `journal: skipping corrupt line {n} ({reason})` — the **same**
  `journal: skipping corrupt line {n} ` prefix as above, followed by the failure
  reason in parentheses — and move on (no entry). (Only the syntactically-invalid
  case pins its exact bytes; here just the shared prefix and the surrounding
  parentheses are fixed, the `{reason}` inside is the deserialize/validation
  message.)
- On success, push the entry.

The two phases matter: a line that is not valid JSON at all must take the
`(invalid JSON)` branch, and only a line that parses as JSON but is not a valid
entry takes the `({reason})` branch. Do not collapse them into one deserialize
attempt — a raw non-JSON line must never surface a serde parse message.

Validation of an entry rejects it when: `gen` is less than 1; `module` is empty;
`target` is empty; or any file's `path` is empty. A rejected entry is skipped
with a warning exactly like a deserialize failure — it never appears in the
returned entries.

## Querying the journal

`next_gen_number(entries: &[JournalEntry]) -> i64`: one past the highest `gen`
among the entries, or `1` for an empty slice.

`file_history(entries: &[JournalEntry], file_path: &str) -> Vec<BlameSourceEntry>`:
in entry order, for each entry that has a file whose `path` equals `file_path`,
emit a `BlameSourceEntry` carrying that entry's `gen`, `timestamp`, `module`, the
matching file's `patch`, and the entry's `prompt_diff` (as `prompt_diff`).
Entries that do not touch the file are omitted, so the result is empty when no
entry touches `file_path`.
