# human-language

## Problem Statement

How might we make prompts the durable source of truth for software, with generated code as an inspectable, locked build artifact — portable across target languages?

## Architecture

```
prompt files (*.hl)             ← what humans read, write, review
        │  hl gen: agentic LLM session (full tool access, writes src directly)
        ▼
src tree (.hl/src/<target>/)    ← real code, written freely by the agent,
        │                          locked read-only after tests pass
        ▼  observed diff
map (.hl/map.json)              ← prompt ↔ file attribution from reality
        │  derived after gen
        ▼
IR (.hl/ir/*.yaml)              ← extracted contract layer: signatures + test
                                   contracts, for review, LSP hover, gating
```

The AI is not constrained to a pipeline. `hl gen` snapshots content hashes, hands the prompt to an agent (claude CLI, agentic mode, cwd = the target src tree — it may scaffold, run package managers, edit many files), then diffs the tree. Changed files are attributed to the driving prompt in `map.json` — the many-to-many mapping is recorded from observed reality, not planned upfront. Tests gate; on green, files lock. The IR is derived from the resulting code as a reviewable contract, not authored as an intermediate build step.

Agent decisions that go beyond the prompt (framework choice, added dependency) are captured in the gen report; `hl reconcile` proposes prompt amendments so prompts stay truthful — the tool never silently rewrites the user's words.

### Layer 1 — Prompt files

Own extension `.hl` (not `.hl.md`) so IDEs bind our grammar and LSP to it. Content is YAML frontmatter (machine-facing metadata) + markdown prose body (the prompt). Syntax highlighting via a shipped TextMate grammar (tree-sitter later) that embeds YAML highlighting for the frontmatter and markdown highlighting for the body, and adds custom scopes on top: `@module/ref` cross-references, test blocks, contract keywords. Cross-ref tokens are what the LSP hangs hover/go-to-definition on.

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

Per-target directory under `.hl/src/<target>/`. Goal is to support as many languages as possible; the IR makes each new language a **target adapter**, not a compiler: a config declaring the test framework, build/run commands, file layout conventions, and idiom guidance for the codegen LLM. Cost per added language should be a config file plus prompt guidance, not engineering months. TypeScript first; a second target (Swift) proves the agnosticism claim. Generated test code is emitted from IR test cases in each target's native framework. Read-only: enforced by `hl status` in CI + local file permissions.

## Mapping

`.hl/map.json` records the graph:

- prompt → IR modules: N:M — a prompt may define several modules, and a later prompt may amend an existing module by declaring `extends: <module>` in frontmatter. Build merges all contributing prompts into that module's IR; contradictions between contributors (e.g. two prompts stating different expiry times) are flagged at IR merge, not silently resolved. Regen of a file always consumes every contributing prompt.
- IR module → src files: 1:N per target, each src file owned by exactly one module
- cross-module references: via the IR import graph (this is where many-to-many lives)

Ownership is 1:N; sharing happens at interface level. Regen blast radius = the owning module's files only.

## Toolchain

- `hl build` — prompts → IR (only for changed prompts)
- `hl gen <target>` — IR → target code, loops until IR test cases pass or fails loudly
- `hl status` — drift check: hidden src hash vs lockfile
- `hl test` — run generated tests in target language
- Lockfile pins model ID; regen with a new model is an explicit, versioned event

## LSP (same milestone, built after CLI core)

- hover on prompt/module → linked IR + src snippet
- hover on any prompt body line → the implementing code ranges (span attribution, regenerated every gen)
- go-to-definition → jump into `.hl/` at exact lines
- **reverse navigation**: from generated src, "Go to prompt" jumps to the causing prompt sentence(s); multiple contributing prompts → peek list (find-usages style); CodeLens above each function names its owning prompt span
- built on `map.json` + `.hl/attribution/*.yaml`

## Two-Way Editing (Reconciliation)

Direct src editing is supported, but only as a staging area for prompt changes — never as a fork. It works because the generated baseline is always committed, so the delta is a deterministic diff, not a reconstruction.

- `hl edit <file>` — explicit unlock: file becomes writable, module marked **patched**. Editing without unlock is still DRIFT (error, CI gate fails).
- The delta vs generated baseline is stored as `.hl/patches/<module>.diff` — byte-exact.
- `hl reconcile <module>` — LLM translates the patch into a proposed prompt/IR amendment (including test-case updates when behavior changed). User approves → prompt updated → regen → regen must reproduce patched behavior (test cases prove it) → patch cleared, file re-locked.
- If both src and prompt changed since last gen: 3-way conflict, surfaced explicitly, never silently merged.

Free-form inverse compilation (guessing a prompt from arbitrary code with no baseline) remains out of scope.

## Decisions Made

- Drift policy: unsanctioned hidden-src edits **forbidden** (DRIFT); deliberate edits go through `hl edit` → patch → `hl reconcile` back into the prompt
- Tests are **core**: part of the prompt frontmatter / IR, regen must pass them
- Attribution is a **hard gate**: if span attribution can't be derived (3 attempts, sanity-checked), gen fails — files stay unlocked, module stays stale, status reports `unattributed` (exit 1). Code without a traceable prompt is never accepted as green.
- Prompt format: markdown + YAML frontmatter
- IR: contract-level YAML, not AST
- First target: TypeScript; Swift as agnosticism proof later

## Key Assumptions to Validate

- [ ] Prompt + tests carry enough intent that regen with a newer model preserves behavior — write 5 modules, regen with two models, diff via tests
- [ ] Contract-level IR is enough for the codegen LLM to produce correct idiomatic code — check Swift output quality once TS pipeline works
- [ ] LLM can reliably maintain `map.json` and the IR import graph during build
- [ ] Devs tolerate read-only src — dogfood 2 weeks on a real small project
- [ ] Hover/jump makes hidden code feel safe — demo to 3 devs

## Not Doing (and Why)

- Back-propagating src edits to prompts — inverse compilation, research problem
- ~~Implicit regen on save~~ REVERSED: saving a `.hl` file triggers incremental gen — LSP didSave → debounce → session receives only the prompt diff + attribution entries for changed lines + affected files, instructed to make minimal edits to owned regions (full scaffold only when no prior gen exists). Tests + attribution gate apply; diagnostics/status-bar surface progress. Extension setting `genOnSave` to disable; manual `hl gen` remains. The original cost objection applied to full regen; diff-scoped sessions remove it.
- AST-level IR / deterministic transpile — Haxe-sized effort, least-common-denominator output
- Multiple targets in MVP — TS proves the pipeline; Swift proves portability, later
- Prompt package registry — 10x version, revisit if core works

## Open Questions

- IR schema versioning — IR format will evolve; migration story for existing `.hl/ir/` files?
- Test cases that can't be expressed as given/expect data (e.g. "retries with backoff") — prose contracts in IR checked how?
- Model pinning vs upgrade path — regen with new model = major version bump?
- Frontmatter test syntax — enough expressiveness, or does it need scenario blocks?
