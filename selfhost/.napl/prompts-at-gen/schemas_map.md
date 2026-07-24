# The NAPL map v2 schema and its pure query/mutation helpers

This module defines the serde types and pure helpers for the NAPL **map** — the
`v2` on-disk index recording, for every prompt, which target files it generated
(with content hashes), and the reverse file→prompt attribution. It is pure: no
I/O; every helper takes data in and returns data or mutates a passed map. Bring
in `serde`, `serde_json`, and the standard library.

## Where this code lives

The working directory is a Cargo workspace whose root manifest is written and
owned by the toolchain — leave it alone. Create this module as its own member
crate in a subdirectory named `schemas_map/`: `schemas_map/Cargo.toml` (package
name `schemas_map`) and `schemas_map/src/lib.rs`. Touch nothing outside
`schemas_map/`. Ensure `cargo test` passes from the workspace root before
finishing.

## Builds on the `schemas_ordered_map` module of this workspace

The map preserves **insertion order** of its prompt keys, file keys, and each
prompt's per-target sub-records. That ordered-map value type is the
**`schemas_ordered_map`** module of this same workspace, generated as a sibling
member crate in the `schemas_ordered_map/` directory (so `../schemas_ordered_map`
relative to this crate). It exposes a public generic type `OrderedMap<V>` — an
insertion-ordered string-keyed map — with `new`, `len`, `is_empty`,
`contains_key`, `get`, `get_mut`, `insert`, `remove`, `iter` (yielding
`(&String, &V)` in insertion order), `keys`, and `values`, plus serde
`Serialize`/`Deserialize` that round-trips as a JSON object preserving key order.

Use that generated sibling crate's `OrderedMap<V>` for every ordered-map field below — do not reimplement an ordered map here, and do not depend on any hand-written crate.

## The types

Expose these public types, all deriving serde serialize + deserialize, equality,
and clone:

- `PromptTargetRecord`: a per-target record inside a prompt entry, with public
  fields:
  - `prompt_hash_at_gen: Option<String>`, JSON key **`promptHashAtGen`**, omitted
    from output when `None`, defaulting to `None` when absent.
  - `files: Vec<String>`, defaulting to empty when absent.
  - `unattributed: Option<bool>`, omitted from output when `None`, defaulting to
    `None` when absent.
- `PromptRecord`: a prompt entry, with public fields:
  - `module: String`.
  - `prompt_hash: String`, JSON key **`promptHash`**.
  - `declared_targets: Vec<String>`, JSON key **`declaredTargets`**, defaulting to
    empty when absent.
  - `targets: OrderedMap<PromptTargetRecord>`, defaulting to empty when absent.
- `FileRecord`: a file entry, with public fields:
  - `target: String`.
  - `hash: String`.
  - `prompts: Vec<String>`, defaulting to empty when absent.
- `NaplMap`: the whole map, with public fields:
  - `version: u32`, defaulting to `2` when absent.
  - `prompts: OrderedMap<PromptRecord>`, defaulting to empty when absent.
  - `files: OrderedMap<FileRecord>`, defaulting to empty when absent.

And these plain data structs (comparable/cloneable; no serde needed):

- `ModuleFile` with public `target: String` and `file_path: String`.
- `FileInput` with public `file_path: String` and `hash: String`.
- `AttributionInput` with public `rel: String`, `module: String`,
  `prompt_hash: String`, `target: String`, `declared_targets: Vec<String>`, and
  `files: Vec<FileInput>`.
- `UnattributedInput` with public `rel: String`, `module: String`,
  `prompt_hash: String`, `target: String`, `declared_targets: Vec<String>`, and
  `files: Vec<String>`.

## Construction and (de)serialization

- `empty_map() -> NaplMap`: version 2, empty prompts, empty files.
- `parse_map(raw: &str) -> Result<NaplMap, _>`: deserialize a JSON string; then
  if `version` is not `2`, it is a validation error ("unsupported map version").
  Corrupt JSON is a deserialization error. Neither panics.
- `map_to_json(map: &NaplMap) -> String`: pretty-print the map as JSON and append
  a single trailing newline. A written map re-parses to an equal map.

## Query helpers

- `prompts_for_module(map, module) -> Vec<String>`: the relative prompt keys
  whose record's `module` equals `module`, in insertion order.
- `has_module(map, module) -> bool`: whether the module has any prompts.
- `declared_targets_for_module(map, module) -> Vec<String>`: the union of the
  `declared_targets` across the module's prompt records, deduplicated, in first-
  seen order.
- `files_for_module(map, module) -> Vec<ModuleFile>`: for each of the module's
  prompts, for each `(target, target_record)` pair in insertion order, each file
  path in `target_record.files`, deduplicated by the `(target, file_path)` pair,
  in first-seen order.
- `is_prompt_gen_stale(record: Option<&PromptRecord>, target, prompt_hash, force) -> bool`:
  `true` if `force`; else `true` if the record is absent; else `true` if the
  record has no target sub-record for `target`; else `true` if that sub-record's
  `unattributed` is `Some(true)`; else `true` if its `prompt_hash_at_gen` is
  absent or differs from `prompt_hash`; otherwise `false`.

## Mutation helpers

Both mutate the passed map in place.

### `record_attribution(map, input: &AttributionInput)`

Records a successful generation. Let `existing_targets` be the current prompt
record's `targets` (if the prompt `rel` already exists), and `prior_files` be the
files the `input.target` sub-record previously listed. For every prior file that
is **not** among the new `input.files` paths, detach it (see "Detaching" below).
Then set the `input.target` sub-record to a `PromptTargetRecord` with
`prompt_hash_at_gen = Some(input.prompt_hash)`, `files` = the new file paths in
order, and `unattributed = None`, keeping any other targets. Store the prompt
record under `input.rel` with `module`, `prompt_hash`, `declared_targets`, and
those `targets`. Finally, for each new file, ensure a `FileRecord` under its path
with `target = input.target`, `hash` = that file's hash, and a `prompts` list
that includes `input.rel` (appended once if not already present, preserving any
existing prompts for a file shared across prompts).

### `record_unattributed(map, input: &UnattributedInput)`

Records a generation that produced files but could not be attributed. As above,
detach **every** prior file of the `input.target` sub-record. Then set that
sub-record to `prompt_hash_at_gen = None`, `files = input.files`, and
`unattributed = Some(true)`, keeping other targets, and store the prompt record
under `input.rel`. Do **not** create `FileRecord` entries for the files.

### Detaching a file from a prompt

To detach `file_path` from prompt `rel`: if the map has a `FileRecord` for
`file_path`, remove `rel` from its `prompts`; if no prompts remain, remove the
file entry entirely; otherwise keep the file entry with the reduced `prompts`.

This is why re-attributing a prompt to a strictly smaller set of files drops the
now-orphaned file entry, and why marking a target unattributed detaches all of
its files. A subsequent successful `record_attribution` for the same prompt/target
overwrites the sub-record, clearing the `unattributed` flag and restoring a
`prompt_hash_at_gen`.
