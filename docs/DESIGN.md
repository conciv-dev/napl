# NAPL — NAPL Ain't a Programming Language

## Problem Statement

How might we make prompts the durable source of truth for software, with generated code as an inspectable, locked build artifact — portable across target languages?

## Architecture

```
prompt files (*.napl)             ← what humans read, write, review
        │  napl gen: agentic LLM session (full tool access, writes src directly)
        ▼
src tree (.napl/src/<target>/)    ← real code, written freely by the agent,
        │                          locked read-only after tests pass
        ▼  observed diff
map (.napl/map.json)              ← prompt ↔ file attribution from reality
        │  derived after gen
        ▼
IR (.napl/ir/*.yaml)              ← extracted contract layer: signatures + test
                                   contracts, for review, LSP hover, gating
```

The AI is not constrained to a pipeline. `napl gen` snapshots content hashes, hands the prompt to an agent (claude CLI, agentic mode, cwd = the target src tree — it may scaffold, run package managers, edit many files), then diffs the tree. Changed files are attributed to the driving prompt in `map.json` — the many-to-many mapping is recorded from observed reality, not planned upfront. Tests gate; on green, files lock. The IR is derived from the resulting code as a reviewable contract, not authored as an intermediate build step.

## Repo Layout

A pnpm + turborepo monorepo (Node >= 22, pnpm pinned via the root `packageManager`).

```
packages/core   @napl/core — the library: prompts, IR, attribution, journal/blame,
                target adapters, and the LLM/agent backends (src/core/ + src/targets/,
                re-exported from a single index barrel)
packages/cli    @napl/cli  — the `napl` binary (init/gen/status/test/blame); depends on
                @napl/core via workspace:*
packages/lsp    @napl/lsp  — the language server (hover, go-to-def, reverse nav,
                gen-on-save); depends on @napl/core
apps/vscode     the VS Code extension; its esbuild step bundles the server from
                @napl/lsp's source into a self-contained dist/server.js. Unpublished.
examples/       greeting.🧑 and todo-app — real projects carrying their own
                committed .napl/ state (emoji + canonical spellings); not workspace packages.
```

turbo tasks: `build` (`dependsOn ^build`, outputs `dist/**`), `typecheck`
(`^build`), `test` (`^build` + `build`). `@napl/cli` and `@napl/lsp` import
`@napl/core` by package specifier, never by relative cross-package path.

Agent decisions that go beyond the prompt (framework choice, added dependency) are captured in the gen report; `napl reconcile` proposes prompt amendments so prompts stay truthful — the tool never silently rewrites the user's words.

### Layer 1 — Prompt files

Own extension `.napl` (not `.napl.md`) so IDEs bind our grammar and LSP to it. Content is YAML frontmatter (machine-facing metadata) + markdown prose body (the prompt). Syntax highlighting via a shipped TextMate grammar (tree-sitter later) that embeds YAML highlighting for the frontmatter and markdown highlighting for the body, and adds custom scopes on top: `@module/ref` cross-references, test blocks, contract keywords. Cross-ref tokens are what the LSP hangs hover/go-to-definition on.

```yaml
---
module: auth/session
deps: [auth/tokens]
targets: [typescript]
tests:
  - name: expired token rejected
    given: { token: expired }
    expect: { error: SESSION_EXPIRED }
---
Manage user sessions. Sessions expire after 30 minutes of inactivity.
Refreshing a session extends it. An expired token is rejected with
SESSION_EXPIRED, never silently renewed.
```

### Layer 2 — IR (the middle language)

YAML. Captures **contracts, not implementation**:

- module name, dependencies (import graph)
- exported types (structural, language-neutral)
- function signatures with behavior contracts (pre/post conditions in prose)
- test cases as data (given/expect pairs)

Explicitly NOT in the IR: control flow, concurrency model, memory idioms, syntax trees. Those are per-target decisions made by the codegen LLM. This avoids the least-common-denominator trap that kills AST-level transpilers.

IR is generated from prompts by LLM, but human-inspectable and diffable — it is the review surface for "did the model understand my prompt."

### Layer 3 — Target code

Per-target directory under `.napl/src/<target>/`. Goal is to support as many languages as possible; the IR makes each new language a **target adapter**, not a compiler: a config declaring the test framework, build/run commands, file layout conventions, and idiom guidance for the codegen LLM. Cost per added language should be a config file plus prompt guidance, not engineering months. TypeScript first; a second target (Swift) proves the agnosticism claim. Generated test code is emitted from IR test cases in each target's native framework. Read-only: enforced by `napl status` in CI + local file permissions.

## Mapping

`.napl/map.json` records the graph:

- prompt → IR modules: N:M — a prompt may define several modules, and a later prompt may amend an existing module by declaring `extends: <module>` in frontmatter. Build merges all contributing prompts into that module's IR; contradictions between contributors (e.g. two prompts stating different expiry times) are flagged at IR merge, not silently resolved. Regen of a file always consumes every contributing prompt.
- IR module → src files: 1:N per target, each src file owned by exactly one module
- cross-module references: via the IR import graph (this is where many-to-many lives)

Ownership is 1:N; sharing happens at interface level. Regen blast radius = the owning module's files only.

## Toolchain

- `napl build` — prompts → IR (only for changed prompts)
- `napl gen <target>` — IR → target code, loops until IR test cases pass or fails loudly
- `napl status` — drift check: hidden src hash vs lockfile
- `napl test` — run generated tests in target language
- Lockfile pins model ID; regen with a new model is an explicit, versioned event

## LSP (same milestone, built after CLI core)

- hover on prompt/module → linked IR + src snippet
- hover on any prompt body line → the implementing code ranges (span attribution, regenerated every gen)
- go-to-definition → jump into `.napl/` at exact lines
- **reverse navigation**: from generated src, "Go to prompt" jumps to the causing prompt sentence(s); multiple contributing prompts → peek list (find-usages style); CodeLens above each function names its owning prompt span
- built on `map.json` + `.napl/attribution/*.yaml`

## Prompt Blame (mechanical line history)

Layered *under* the semantic attribution is a mechanical, git-blame-style record
of how each line of generated code came to be. Every successful `napl gen` for a
module appends one JSON line to an append-only journal, `.napl/journal.jsonl`:

- `gen` (monotonic int), `timestamp` (taken from a clock injected at the CLI
  entry, never from library code), `module`, `target`, `promptHash`, and `mode`
  (`full` | `incremental`).
- `promptDiff` — the unified diff of the prior→current prompt body (empty string
  for the first gen or an unchanged prompt).
- `files` — for every changed file: a unified `patch`, plus `hashBefore`
  (`null` when the file did not yet exist) and `hashAfter`. A file's **first**
  appearance in the journal is recorded as a creation patch so blame can always
  replay from an empty file. Prior contents are snapshotted before the agent runs
  and the journal keeps a patch only for the files that actually changed. Reads
  are Zod-validated and corrupt lines are skipped with a warning.

`napl blame <file>` reconstructs line ancestry by replaying that file's journal
patches oldest→newest, tracking which gen last touched each current line — the
same algorithm class as `git blame` (untouched lines keep the oldest gen; a line
moved down by an insertion above keeps its gen; a modified line moves to the
editing gen). `--line N` scopes to one line; `--verbose` adds the prompt edit
(the "why"); `napl blame --gen N` prints a single gen's summary (module, prompt
diff, files touched). The pure blame algorithm lives in
`packages/core/src/core/blame.ts`; the journal format and I/O in
`packages/core/src/core/journal.ts`.

In the editor the mechanical layer surfaces alongside the semantic one: the
generated-file hover and the reverse-navigation CodeLens gain a line
`caused by gen #N · <date> · prompt edit: <first line of the diff hunk>` (or
`initial generation`), shown **first**, with the semantic "implements sentence…"
content kept beneath it — computed via the blame core against the journal.
Pre-journal repos (no `.napl/journal.jsonl`) simply omit the mechanical line.

## The Machine Layer (`.mapl`)

Every prompt module has a hidden-but-referencable machine counterpart,
`.napl/mapl/<module>.mapl` — the LLM's side of the dialogue, regenerated on every
compile. Entries carry `promptLines`, a `kind`, a human-facing `message`, the
model's `reasoning`, and optionally a suggested prompt rewording.

- `ambiguity` → red squiggle on the exact prompt words, like a syntax error.
  Hover shows what was unclear, what the model assumed, and a suggested fix.
- `assumption` → warning squiggle: a decision the prompt didn't specify.
- `note` → hover-visible reasoning about why the code is shaped as it is.
- `no-op` → warning: the prompt changed but the agent made no code change; the
  entry must explain why. A changed prompt with an empty diff and no no-op
  explanation FAILS the gen — silent "clean" without implementing the change is
  forbidden.

The compile is a dialogue: the human writes English, the machine answers in the
margin, and ambiguity is a first-class compile diagnostic.

### Extensions

Dual scheme on both sides of the dialogue, shipped: human files are `.napl`
canonically with person-emoji aliases (`.🧑`, `.🧓`, `.👤`, `.👨`, `.👩`, `.🧒`
— a curated single-codepoint list, overridable via a `promptAliases` array in
`.napl/lock.json`; ZWJ sequences excluded for filesystem safety); machine files
are `.mapl` canonically with the `.🤖` alias. Both spellings are byte-identical
formats and fully equivalent to all tooling — discovery
(`packages/core/src/core/paths.ts`: `isPromptFile`, `promptExtensions`,
`machineExtensions`), the LSP, the grammar, and the VS Code extension register
every alias.

**The mirror rule.** A module's prompt may use any alias; the module name and
all derived state (`.napl/` layouts, `map.json` prompt paths, the journal) record
the actual filename. The machine file `napl gen` writes stays canonical `.mapl`
unless the prompt file uses an emoji alias, in which case it is written as `.🤖`
— the machine mirrors the human's choice of spelling. Reading always accepts both
spellings on both sides.

## Two-Way Editing (Reconciliation)

Direct src editing is supported, but only as a staging area for prompt changes — never as a fork. It works because the generated baseline is always committed, so the delta is a deterministic diff, not a reconstruction.

- `napl edit <file>` — explicit unlock: file becomes writable, module marked **patched**. Editing without unlock is still DRIFT (error, CI gate fails).
- The delta vs generated baseline is stored as `.napl/patches/<module>.diff` — byte-exact.
- `napl reconcile <module>` — LLM translates the patch into a proposed prompt/IR amendment (including test-case updates when behavior changed). User approves → prompt updated → regen → regen must reproduce patched behavior (test cases prove it) → patch cleared, file re-locked.
- If both src and prompt changed since last gen: 3-way conflict, surfaced explicitly, never silently merged.

Free-form inverse compilation (guessing a prompt from arbitrary code with no baseline) remains out of scope.

## Decisions Made

- Drift policy: unsanctioned hidden-src edits **forbidden** (DRIFT); deliberate edits go through `napl edit` → patch → `napl reconcile` back into the prompt
- Tests are **core**: part of the prompt frontmatter / IR, regen must pass them
- Attribution is a **hard gate**: if span attribution can't be derived (3 attempts, sanity-checked), gen fails — files stay unlocked, module stays stale, status reports `unattributed` (exit 1). Code without a traceable prompt is never accepted as green.
- Prompt format: markdown + YAML frontmatter
- IR: contract-level YAML, not AST
- First target: TypeScript; Swift as agnosticism proof later
- Identity: renamed to **NAPL** ("NAPL Ain't a Programming Language"). Canonical extensions are now `.napl` (human) and `.mapl` (machine); hidden state lives in `.napl/` with machine notes under `.napl/mapl/`; the CLI binary and npm scope are `napl` / `@napl/*`.

## Conformance (the port gate)

`conformance/` (`@napl/conformance`, unpublished) is a declarative golden-fixture
corpus that freezes the toolchain's observable contract: for each scenario it
pins exit code, exact stdout/stderr (with `{{CWD}}` path normalization and
`re:`-marked regex lines), and the full byte content / file mode of the small
state files (`map.json`, `journal.jsonl`, `.napl/attribution/*`, `.napl/mapl/*`,
`.napl/ir/*`, `lock.json`). Scenarios run the **actual built CLI** end-to-end in a
throwaway temp dir; determinism comes from a stub `claude`/`npx` on `PATH`
(`conformance/fake-claude/`, driven by a per-scenario script), a `NAPL_FIXED_NOW`
clock override injected only at the CLI entry, and a runner (`conformance/runner/`)
that spawns the binary, normalizes volatile values, and diffs against
expectations. The gen-lock contention case seeds the lock with the runner's own
live pid so the collision is reproducible.

This suite is the **acceptance gate for the Rust rewrite**. The port is expected
to run the same `conformance/scenarios/*.yaml` fixtures through an equivalent
runner and reproduce every pinned value byte-for-byte; any divergence is a port
bug. Nondeterminism discovered while building the corpus (anything that could not
be pinned) is therefore a first-class portability finding, not a test nuisance.

## Key Assumptions to Validate

- [ ] Prompt + tests carry enough intent that regen with a newer model preserves behavior — write 5 modules, regen with two models, diff via tests
- [ ] Contract-level IR is enough for the codegen LLM to produce correct idiomatic code — check Swift output quality once TS pipeline works
- [ ] LLM can reliably maintain `map.json` and the IR import graph during build
- [ ] Devs tolerate read-only src — dogfood 2 weeks on a real small project
- [ ] Hover/jump makes hidden code feel safe — demo to 3 devs

## Not Doing (and Why)

- Back-propagating src edits to prompts — inverse compilation, research problem
- ~~Implicit regen on save~~ REVERSED: saving a `.napl` file triggers incremental gen — LSP didSave → debounce → session receives only the prompt diff + attribution entries for changed lines + affected files, instructed to make minimal edits to owned regions (full scaffold only when no prior gen exists). Tests + attribution gate apply; diagnostics/status-bar surface progress. Extension setting `genOnSave` to disable; manual `napl gen` remains. The original cost objection applied to full regen; diff-scoped sessions remove it.
- AST-level IR / deterministic transpile — Haxe-sized effort, least-common-denominator output
- Multiple targets in MVP — TS proves the pipeline; Swift proves portability, later
- Prompt package registry — 10x version, revisit if core works

## Open Questions

- IR schema versioning — IR format will evolve; migration story for existing `.napl/ir/` files?
- Test cases that can't be expressed as given/expect data (e.g. "retries with backoff") — prose contracts in IR checked how?
- Model pinning vs upgrade path — regen with new model = major version bump?
- Frontmatter test syntax — enough expressiveness, or does it need scenario blocks?
