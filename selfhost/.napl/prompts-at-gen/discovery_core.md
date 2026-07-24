# Location-free prompt identity: the pure decisions

A NAPL prompt is identified by its frontmatter `module` name, never by its path on
disk. This module carries the **pure decisions** the CLI's discovery shell makes
over the prompt files it has already read from disk: reading a prompt's declared
member crate, rejecting two prompts that declare the same module, and building the
`module -> current path` index. The filesystem walk and the reads stay in the
hand-written shell; this module receives the already-read `(raw_contents,
relative_path)` pairs and decides. It is otherwise pure — no filesystem, no
environment.

## Where this code lives

The working directory is a Cargo workspace whose root manifest is written and
owned by the toolchain — leave it alone. Create this module as its own member
crate in a subdirectory named `discovery_core/`: `discovery_core/Cargo.toml`
(package name `discovery_core`) and `discovery_core/src/lib.rs`. Touch nothing
outside `discovery_core/`. Ensure `cargo test` passes from the workspace root
before finishing.

## Builds on one module of this workspace

- **`schemas_frontmatter`** (`../schemas_frontmatter`): exposes
  `parse_frontmatter(raw: &str) -> Result<ParsedPrompt, _>` where `ParsedPrompt`
  has a public `frontmatter: Frontmatter` field, and `Frontmatter` has public
  fields `module: String` and `crate_name: Option<String>`. Use its public API —
  do not reimplement its types or logic, and do not depend on any hand-written
  crate. A prompt whose frontmatter fails to parse is treated as carrying no
  usable identity (see each function below).

## `declared_crate(raw)`

`declared_crate(raw: &str) -> Option<String>`: parse the prompt's frontmatter and
return its `crate_name`. If the frontmatter fails to parse, or there is no `crate`
key, return `None`. So `crate: shared` in the frontmatter yields `Some("shared")`,
an absent key yields `None`, and unparseable input yields `None`.

## `find_duplicate_module(files)`

`find_duplicate_module(files: &[(String, String)]) -> Option<String>`: each tuple
is `(raw_contents, relative_path)` for one prompt file, in discovery order. Walk
them in order, parsing each prompt's frontmatter; **skip** any whose frontmatter
fails to parse (its parse error is surfaced elsewhere). Track the first relative
path seen for each module name. The moment a module name is seen a second time,
return the error message (and stop):

    duplicate module '{module}' is declared by two prompts: {existing} and {new}. Copied a prompt? Rename the module in the copy.

where `{module}` is the repeated module name, `{existing}` is the relative path
first recorded for it, and `{new}` is the relative path of the current (second)
prompt. If no module repeats, return `None`. Identity is the module, so a copied
prompt must be renamed in the copy — two prompts sharing a module is a hard error,
never a silent fork.

## `module_paths_from(files)`

`module_paths_from(files: &[(String, String)]) -> std::collections::BTreeMap<String, String>`:
each tuple is `(raw_contents, relative_path)`. Build the derived `module ->
relative_path` index: parse each prompt's frontmatter, skipping any that fails to
parse, and insert `module -> relative_path` into a `BTreeMap`. When two files
declare the same module (the caller has already rejected that via
`find_duplicate_module` before calling this), a later entry overwrites an earlier
one. This is how a prompt's location is resolved now that identity is keyed by
module and no path is stored: the path is always the one found on disk this run.
