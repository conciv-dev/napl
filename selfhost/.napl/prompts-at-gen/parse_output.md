# Extracting YAML from an agent response

This module pulls the YAML payload out of a language-model response that may wrap
it in a Markdown code fence. It is pure: no I/O, no dependencies on other project
modules.

## Where this code lives

The working directory already contains a generated `body_lines` crate at its
root — leave it completely untouched. Create this module as a **new, separate
Rust library crate in a subdirectory named `parse_output/`**:
`parse_output/Cargo.toml` (package name `parse_output`) and
`parse_output/src/lib.rs`. Do not add a workspace manifest and do not modify
anything outside `parse_output/`. Ensure `cargo test` passes from inside
`parse_output/`.

## `extract_yaml(text)`

Return the YAML content contained in `text`.

Look for the first Markdown code fence: a line beginning with three backticks.
The fence's opening line may carry an info string (`yaml`, `yml`, or nothing) and
possibly other characters; everything on that opening line up to and including its
terminating newline is discarded. The content is then everything from the start of
the next line up to (but not including) the next occurrence of three backticks.
Return that content with surrounding whitespace trimmed.

If there is no complete fenced block — no opening fence, or an opening fence with
no closing three backticks after a newline — return the whole `text` trimmed of
surrounding whitespace instead.

Examples:

- A block opening with three backticks then `yaml`, containing
  `module: greeting` and `tests: []` on two lines, yields exactly
  `module: greeting\ntests: []`.
- A bare fence (three backticks, newline, `[]`, newline, three backticks) yields
  `[]`.
- Plain text with leading and trailing spaces and no fence yields that text
  trimmed.
</content>
