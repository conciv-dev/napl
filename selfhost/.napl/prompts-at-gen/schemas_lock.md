# The lock document: model / backend / agent / prompt-alias configuration

This module defines the serde types and validators for the NAPL lock document —
the small JSON configuration that pins which model, code-generation backend,
coding-agent engine, and prompt aliases the toolchain compiles through. It is
pure: no I/O. Bring in `serde` and `serde_json`.

## Where this code lives

The working directory is a Cargo workspace whose root manifest is written and
owned by the toolchain — leave it alone. Create this module as its own member
crate in a subdirectory named `schemas_lock/`: `schemas_lock/Cargo.toml`
(package name `schemas_lock`) and `schemas_lock/src/lib.rs`. Touch nothing
outside `schemas_lock/`. Ensure `cargo test` passes from the workspace root
before finishing.

## Builds on the `extensions` module of this workspace

This module builds on the **`extensions`** module of this same workspace. The
`extensions` module is generated as a sibling member crate in the `extensions/`
directory (so `../extensions` relative to this crate). It exposes a public
function `default_prompt_aliases() -> Vec<String>` returning the curated default
list of prompt-alias strings.

Add a path dependency on that sibling member crate in your `Cargo.toml`
(depend on the `extensions` crate by path — it lives one directory up and over,
at `../extensions`) and call its `default_prompt_aliases()` for the curated
default below. Do **not** reimplement the curated alias list here, and do not
depend on any hand-written crate — depend only on the generated sibling
`extensions` member crate. This sibling path dependency is part of the same
workspace, not an external crates.io dependency, so add it even though the
general guidance is to avoid outside dependencies.

## The value types

Expose these public types, deriving the serde traits noted:

- `Backend`: an enum with two variants, `ClaudeCli` and `AnthropicApi`,
  serialized/deserialized in **kebab-case** (so the JSON spellings are
  `"claude-cli"` and `"anthropic-api"`). It derives serialize and deserialize,
  and is comparable and copyable.
- `AgentPreset`: an enum with three variants, `Claude`, `Codex`, and `Custom`,
  serialized/deserialized in **lowercase** (`"claude"`, `"codex"`, `"custom"`).
- `AgentConfig`: a struct with a public `preset: AgentPreset` and an optional
  public `command: Option<Vec<String>>` (a command template; omitted from
  serialized output and defaulting to `None` when absent). It **denies unknown
  fields** — any JSON key other than `preset` and `command` is a deserialization
  error.
- `HlLock`: the lock document itself, with a public `model: String`; a
  `backend: Backend` that defaults to claude-cli when the key is absent; an
  optional `prompt_aliases: Option<Vec<String>>` whose JSON key is
  **`promptAliases`** (camelCase), omitted when absent; and an optional
  `agent: Option<AgentConfig>` omitted when absent.

Expose these public constants and defaults:

- `DEFAULT_MODEL: &str` equal to `"claude-sonnet-5"`.
- `DEFAULT_BACKEND: Backend` equal to the claude-cli variant.
- `DEFAULT_AGENT_PRESET: AgentPreset` equal to the claude variant.
- `default_agent_config() -> AgentConfig`: the claude preset with no command.

## Parsing and validating a lock

Expose `parse_lock(raw: &str) -> Result<HlLock, _>` that deserializes a JSON
string into an `HlLock` and then validates it. Deserialization failures (corrupt
JSON, unknown backend spelling, unknown preset, unknown agent field, wrong
types) are errors, not panics. After a structural parse, enforce:

- `model` must not be empty — an empty model string is a validation error.
- If `promptAliases` is present, every alias must be valid (see below).
- If `agent` is present, it must be valid (see below).

### Prompt-alias validation

A prompt alias is a short string. Validate each one:

- It must start with a `.` (dot). An alias not beginning with `.` is rejected.
- After the leading dot it must have **1 or 2 Unicode code points** — no more,
  no fewer. So `".abc"` (three code points after the dot) is rejected.
- It must not contain a ZWJ (zero-width joiner, U+200D) anywhere. A ZWJ-joined
  emoji sequence such as `".👨‍💻"` (man + ZWJ + laptop) is rejected even though it
  is visually one glyph.

The exact error wording does not matter — only which aliases are accepted versus
rejected. A valid override list is kept verbatim.

### Agent-config validation

Validate the agent configuration by preset:

- The `custom` preset **requires** a non-empty `command` array. A custom preset
  with no `command`, or with an empty `command`, is a validation error.
- The `claude` and `codex` presets **must not** carry a `command`. Either of
  those presets with a `command` present is a validation error.

## Resolving effective configuration

Expose two resolvers:

- `resolve_prompt_aliases(lock: &HlLock) -> Vec<String>`: the lock's
  `promptAliases` override if present, otherwise the curated default obtained by
  calling the `extensions` module's `default_prompt_aliases()`.
- `resolve_agent_config(lock: &HlLock) -> AgentConfig`: the lock's `agent`
  override if present, otherwise `default_agent_config()` (the claude preset).
