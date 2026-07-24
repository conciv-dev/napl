# The machine-layer (mapl) schema: model annotations keyed to prompt lines

This module defines the serde types and validators for a NAPL **machine-layer
document** (the `.mapl` sidecar): the list of model-authored annotations that
flag, for a range of prompt body lines, an ambiguity, an assumption, a note, or a
no-op. It is pure: no I/O. Bring in `serde`, `serde_json`.

## Where this code lives

The working directory is a Cargo workspace whose root manifest is written and
owned by the toolchain — leave it alone. Create this module as its own member
crate in a subdirectory named `schemas_ml/`: `schemas_ml/Cargo.toml` (package
name `schemas_ml`) and `schemas_ml/src/lib.rs`. Touch nothing outside
`schemas_ml/`. Ensure `cargo test` passes from the workspace root before
finishing.

## Builds on the `schemas_line_range` module of this workspace

Every annotation is keyed to an inclusive 1-based line range that accepts several
JSON spellings (a bare number `n` meaning `n..=n`, a one-element array `[n]`
meaning `n..=n`, or a two-element array `[a, b]`), each endpoint an integer at
least 1. That value type — and its lenient deserializer — is the
**`schemas_line_range`** module of this same workspace, generated as a sibling
member crate in the `schemas_line_range/` directory (so `../schemas_line_range`
relative to this crate). It exposes a public type `LineRange` with public `start`
and `end` fields (`u32`), a `new(start, end)` constructor, equality, copy, and a
serde `Deserialize` performing exactly that normalization.

Use that generated sibling crate's `LineRange` for the line-range field below — do not reimplement line-range parsing here, and do not depend on any hand-written crate.

## The types

Expose these public types:

- `MlKind`: an enum of four annotation kinds, deserializable, comparable, copy:
  `Ambiguity`, `Assumption`, `Note`, and `NoOp`. Their JSON spellings are
  **kebab-case**: `"ambiguity"`, `"assumption"`, `"note"`, and — for `NoOp` — the
  literal `"no-op"`.
- `MlEntry`: one annotation, deserializable, comparable, cloneable, with public
  fields:
  - `prompt_lines: LineRange` whose JSON key is **`promptLines`** (camelCase).
  - `kind: MlKind`.
  - `message: String`.
  - `reasoning: String` defaulting to the empty string when absent.
  - `suggestion: Option<String>` defaulting to `None` when absent.
- `Ml`: a whole document, deserializable, comparable, cloneable, with public
  fields:
  - `module: String`.
  - `target: String`.
  - `entries: Vec<MlEntry>` defaulting to empty when absent.

## Validation

- `validate_ml(value: serde_json::Value) -> Result<Ml, _>`: deserialize the JSON
  value into an `Ml`, then validate. `module` must not be empty, `target` must
  not be empty, and every entry's `message` must not be empty. A structurally
  invalid value — an unknown `kind` such as `"bogus"`, or a `promptLines` that is
  a string — is a deserialization error. Rejection is an error, not a panic. A
  scalar `promptLines` such as `18` normalizes to `18..=18`; an absent
  `suggestion` is `None`.
- `parse_ml_entries(value: serde_json::Value) -> Result<Vec<MlEntry>, _>`: if the
  value is **not** a JSON array, return an empty list (no error). Otherwise
  deserialize it as a list of entries and validate each entry's non-empty
  `message`. A malformed list is an error.

## Lookup

- `ml_entries_at_body_line(ml: &Ml, body_line: u32) -> Vec<&MlEntry>`: return
  references to every entry whose `promptLines` range covers `body_line`
  (inclusive), in document order.

For example, given entries `{promptLines:[1,2], message:"a"}` and
`{promptLines:[5,7], message:"b"}`, body line 6 yields only "b", body line 1
yields only "a", and body line 4 yields none.
