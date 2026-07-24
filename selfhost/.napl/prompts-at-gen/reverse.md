# Reverse navigation: from generated code back to the prompt sentence

This module maps a location in a generated source file back to the prompt
sentence(s) that produced it, using already-loaded attribution data. Every
function is side-effect-free; there is no I/O. It ports the TypeScript LSP
`reverse.ts` helpers.

## Where this code lives

The working directory is a Cargo workspace whose root manifest is written and
owned by the toolchain — leave it alone. Create this module as its own member
crate in a subdirectory named `reverse/`: `reverse/Cargo.toml` (package name
`reverse`) and `reverse/src/lib.rs`. Touch nothing outside `reverse/`. Ensure
`cargo test` passes from the workspace root before finishing.

## Builds on three modules of this workspace

This module builds on three sibling member crates of this same workspace,
generated in directories named after each module. Add a path dependency on each
in your `Cargo.toml` (depend on each crate by path — each lives one directory up
and over from this crate) and use their public items directly. Do **not**
reimplement any of them, and do not depend on any hand-written crate — depend
only on these generated sibling member crates. These sibling path dependencies
are part of the same workspace, not external crates.io dependencies, so add them
even though the general guidance is to avoid outside dependencies.

- **`schemas_line_range`** (`../schemas_line_range`): exposes `LineRange`, an
  inclusive 1-based range with public `start`/`end` fields (`u32`) and a
  `new(start, end)` constructor, comparable and copyable. Use it for every line
  range below.
- **`schemas_attribution`** (`../schemas_attribution`): exposes
  `AttributionEntry`, a mapping entry with public fields `prompt_lines: LineRange`
  (JSON key `promptLines`), `file: String`, `lines: LineRange`, and
  `note: String`. Use this exact type for attribution entries — do not define your
  own. (That crate already depends on `schemas_line_range` for its `LineRange`, so
  the `LineRange` you use is the same type.)
- **`body_lines`** (`../body_lines`): exposes `PromptBody`, a prompt split into
  frontmatter and body, with a public `body_start_line: usize` field (the 0-based
  document line at which the body begins). Use it for `match_prompt_lines` below.

## Constants

Expose two public string constants:

- `GENERATED_PREFIX: &str` equal to `".napl/src/"` — the prefix every generated
  file path carries.
- `DRIFT_LENS_PREFIX: &str` equal to
  `"DRIFT — edits here are not reflected in any prompt"`.

## Splitting a generated path

`GeneratedPathInfo` is a public struct (comparable, cloneable) with public
`target: String` and `target_rel_path: String`.

`parse_generated_path(rel_full: &str) -> Option<GeneratedPathInfo>`: normalize
backslashes to forward slashes; strip the `GENERATED_PREFIX`; then the text up to
the first `/` is the `target` and the remainder is the `target_rel_path`. Return
`None` if the prefix is absent, if there is no `/` after it, if the target would
be empty (a leading `/`), or if the target-relative path would be empty. So
`.napl/src/typescript/src/greeting.ts` → target `typescript`, path
`src/greeting.ts`; `.napl/src/typescript` (no trailing path) → `None`;
`examples/greeting.napl` and `src/greeting.ts` → `None`.

## Attribution sources and matches

- `AttributionSource` is a public struct (comparable, cloneable) with public
  fields `module: String`, `target: String`, `entries: Vec<AttributionEntry>`,
  and `prompt_files: Vec<String>` — one module's attribution together with the
  prompt files contributing to it.
- `ReverseMatch` is a public struct (comparable, cloneable) with public fields
  `module: String`, `target: String`, `prompt_file: String`, `note: String`,
  `prompt_lines: LineRange`, and `code_lines: LineRange` — one code range and the
  prompt span behind it.

`reverse_matches(sources: &[AttributionSource], target: &str, target_rel_path: &str, code_line: Option<u32>) -> Vec<ReverseMatch>`:
for each source whose `target` equals `target`, for each entry whose `file`
equals `target_rel_path`, and (when `code_line` is `Some`) whose `lines` range
contains that line inclusive, emit **one match per prompt file** in
`prompt_files`, carrying the source's module/target, that prompt file, the
entry's `note`, `prompt_lines`, and (as `code_lines`) the entry's `lines`. A
`None` `code_line` returns all entries for the file regardless of line. Order
follows sources, then entries, then prompt files.

## Line-range conversions

- `prompt_absolute_lines(body_start_line: usize, prompt_lines: LineRange) -> (usize, usize)`:
  convert body-relative 1-based prompt lines to absolute 0-based document lines —
  `(body_start_line + start - 1, body_start_line + end - 1)`. So
  `(12, [7,7]) -> (18, 18)`, `(12, [3,4]) -> (14, 15)`, `(3, [1,1]) -> (3, 3)`.
- `match_prompt_lines(body: &PromptBody, prompt_lines: LineRange) -> (usize, usize)`:
  the same conversion using `body.body_start_line`.

## Drift, titles, dedupe

- `is_file_drifted(recorded_hash: Option<&str>, actual_hash: &str) -> bool`: when
  a hash was recorded, whether it differs from the actual hash; when none was
  recorded, `false`.
- `code_lens_title(prompt_basename: &str, absolute_line: usize, note: &str) -> String`:
  when `note` is empty, `"⇠ {prompt_basename}:{absolute_line}"`; otherwise
  `"⇠ {prompt_basename}:{absolute_line} — {note}"`. The leading glyph is U+21E0
  (leftwards dashed arrow) and the separator is space, em-dash (U+2014), space.
- `dedupe_matches(matches: &[ReverseMatch]) -> Vec<ReverseMatch>`: keep the first
  match for each distinct `(prompt_file, prompt_lines.start, prompt_lines.end)`
  span, dropping later duplicates, preserving order.
