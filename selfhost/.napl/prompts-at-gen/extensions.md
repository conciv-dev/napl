# Prompt and machine file extensions

This module owns the file-extension vocabulary of NAPL: which suffixes mark a
file as a *prompt* (human-authored source) and which mark a *machine* (compiled)
file, with matching done correctly over whole Unicode codepoints. It is pure: no
I/O, no dependencies.

## Where this code lives

The working directory already contains a generated `body_lines` crate at its
root — leave it completely untouched. Create this module as a **new, separate
Rust library crate in a subdirectory named `extensions/`**: `extensions/Cargo.toml`
(package name `extensions`) and `extensions/src/lib.rs`. Do not add a workspace
manifest and do not modify anything outside `extensions/`. Ensure `cargo test`
passes from inside `extensions/`.

## The canonical extensions

Expose these as public constants (string slices):

- The canonical **prompt** extension is `.napl`.
- The canonical **machine** extension is `.mapl`.
- The **machine alias** is a dot followed by the robot-face emoji (Unicode
  U+1F916). It is the emoji spelling of a compiled file.

## The curated prompt aliases

Besides `.napl`, a prompt file may be spelled with a dot followed by exactly one
of six curated "single person" emoji. In codepoint order they are: person
(U+1F9D1), older person (U+1F9D3), bust in silhouette (U+1F464), man (U+1F468),
woman (U+1F469), child (U+1F9D2). Expose this curated set, and a function
`default_prompt_aliases()` returning them as owned strings in that order.

## Recognized extension lists

- `prompt_extensions(aliases)` takes an optional list of alias strings. It returns
  the canonical `.napl` first, followed by either the given aliases (when a list is
  supplied) or the six curated aliases (when none is supplied), in order.
- `machine_extensions()` returns the canonical machine extension followed by the
  emoji machine alias: `.mapl` then `.` + robot emoji.

## Classifying a path

- `is_prompt_file(path, aliases)` is true when `path` ends with any recognized
  prompt extension, using the same optional-alias rule as `prompt_extensions`.
- `is_machine_file(path)` is true when `path` ends with any recognized machine
  extension.

Matching is a plain **suffix** test against each recognized extension string.
Because the extensions are whole codepoints and the suffix test is exact, a
composite emoji sequence never matches a single-person alias: a file ending in
man + zero-width-joiner (U+200D) + laptop (U+1F4BB) is **not** a prompt file,
because that four-scalar sequence is not one of the curated aliases and does not
end with any of them. Likewise the robot-emoji spelling is a machine file, never a
prompt file.

When an explicit alias override is supplied, only the canonical `.napl` and the
override entries match — the curated aliases no longer apply.

## Mirroring the spelling of a compiled file

`machine_extension_for_prompt(prompt_path)` chooses the machine extension whose
spelling mirrors the prompt's. When `prompt_path` ends with the canonical `.napl`,
the machine extension is the canonical `.mapl`. For any other (emoji) prompt
spelling, it is the emoji machine alias. The return type is a `&'static str`.
</content>
