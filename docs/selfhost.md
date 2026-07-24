# Self-Host Pilot — proving the stage0 loop

The endgame for NAPL is to rewrite its own toolchain as `.napl` modules that
target Rust, with the current hand-written Rust binary acting as **stage0** — the
compiler that generates the next toolchain from prompts. This pilot proves the
loop end-to-end on one small, self-contained module before committing to the
full rewrite. It is a proof, not the rewrite.

## The stage0 loop, as run

```
selfhost/body_lines.napl            ← behavior prose + the module's tests as given/expect data
        │  stage0 binary: napl gen rust  (claude-cli agent, cwd = .napl/src/rust/)
        ▼
selfhost/.napl/src/rust/            ← real Rust crate the agent wrote, locked 0444 after cargo test
        │  gate: cargo test  (the rust target adapter's test command)
        ▼
attribution / ir / mapl / journal   ← derived; napl status → clean
        │
        ▼
selfhost/equivalence/               ← the hand-written module's exact unit-test corpus, replayed
                                       against the generated crate  →  5/5 pass
```

Everything above is driven by the same stage0 binary the conformance corpus
gates (`rust/target/release/napl`). The only toolchain change the pilot required
was making `rust` a first-class target.

## What the pilot added to the toolchain

- **A `rust` target adapter** (`napl-core/src/targets.rs`): idiom guidance for a
  Rust library crate, agent tools scoped to `cargo`/`rustc`/`rustfmt`, attribution
  excludes for `target/`, `Cargo.lock`, and the guard docs, and a test command of
  `cargo test`. The adapter now carries its test invocation as data
  (`test_run: TestRunCommand`) instead of a hard-coded `npx vitest run`, so each
  target names its own gate.
- **`.rs` as a recognized source extension** (`cmd_gen.rs`) so generated Rust is
  numbered for IR and span attribution, exactly like TypeScript.
- **A deterministic `cargo` gate stub** (`conformance/fake-claude/cargo`) mirroring
  the `npx` stub, plus scenario `74-gen-rust-target.yaml` — a fake-agent happy path
  proving the rust adapter drives the full gen/attribution/lock loop. Real-LLM gen
  is not corpus material; the corpus stays deterministic.
- **`starter_targets()` vs `list_targets()`**: `napl init` pre-scaffolds guard
  dirs only for the starter set (`typescript`, `react`) so the 39 existing
  conformance scenarios stay byte-identical. `rust` is fully usable — its guard
  docs are written on first `napl gen rust` — it is simply not scaffolded up front.

## Chosen pilot module: `body_lines`

`body_lines` (frontmatter/body line-offset math) was picked over `extensions`
because it is more self-contained: pure string/number logic with no unicode
constants, no configuration parameters, and no cross-module aliases. Its contract
is three functions and one struct — small enough to describe in a page of prose,
rich enough to have real edge cases (CRLF stripping, empty body, out-of-range and
negative document lines).

The prompt (`selfhost/body_lines.napl`) is behavior prose plus the module's six
unit tests expressed as `given`/`expect` data in frontmatter. It is **not** a copy
of the Rust source: it names the public API and describes what each function must
do, and leaves every implementation choice to the agent.

## Pilot results

`napl gen rust` converged on **attempt 1 of 3**:

- The agent scaffolded a `body_lines` crate (`Cargo.toml` + `src/lib.rs`, 195
  lines incl. its own 15-test suite) and `cargo test` passed first try.
- Span attribution derived **9 mappings** (prompt lines → code ranges) and locked
  both source files `0444`.
- The machine layer recorded **3 entries** — all `assumption`/`note`, honestly
  flagging what the prose left open: the implementation language, the integer type
  of `body_start_line`, and the `Option<u64>` return type with checked arithmetic.
  These are exactly the margin-notes the compile-as-dialogue model wants.
- `napl status` → `clean`; the journal recorded gen #1.

The one behavioral gap the prose left open (return type) is visible in the mapl,
not hidden — the machine layer surfaced it rather than silently diverging.

## Equivalence evidence

The equivalence gate (`selfhost/equivalence/`) is a separate cargo project that
depends on the generated crate by path and runs the hand-written `napl-core`
`body_lines` corpus — the exact same input→output cases the hand-written module
asserts for itself — against the generated code.

| Hand-written test case | Result |
| --- | --- |
| `locates_body_start_after_frontmatter` | pass |
| `maps_doc_lines_to_body_lines` (incl. out-of-range → None) | pass |
| `numbers_lines_1_based` | pass |
| `no_frontmatter_treats_all_as_body` | pass |
| `empty_body_after_frontmatter` | pass |

**5/5 pass.** The one signature difference — generated
`body_line_for_doc_line` returns `Option<u64>` where the hand-written returns
`Option<usize>` — is bridged by comparing the numeric values. This is the whole
point: the fixpoint is **behavioral, not byte- or type-identical**. Two
independently-written implementations of the same prompt agree on every observable
case.

## The escape-hatch list

Not every module will gen cleanly. Scaling therefore keeps a shrinking
**escape-hatch list**: modules that stay hand-written because the current
stage0 + prompt cannot reproduce their behavior under the equivalence gate. A
module leaves the list only when its prompt drives a generation that passes its
corpus. The list is the honest measure of how much of the toolchain actually
self-hosts; the goal is to shrink it, never to hide a failure by pinning
byte-output.

## What scaling to the full toolchain requires

1. **Module ordering.** Gen modules in dependency order (leaves first:
   `body_lines`, `extensions`, `hash`, … before `gen`/`cli`). A generated module
   used by a later gen must already pass its equivalence gate. The pilot proved a
   leaf; the next step is a small dependency chain.
2. **A corpus-equivalence fixpoint, defined precisely.** The toolchain
   self-hosts when, for every non-escape-hatch module, the stage0-generated code
   passes that module's hand-written unit-test corpus (the definition this pilot
   exercised). Byte-identity is explicitly not required and not sought. Reaching
   the fixpoint means the generated toolchain can regenerate itself and still pass
   every corpus — the same class of gate the conformance suite already is for the
   CLI's observable contract.
3. **Corpus harnessing at scale.** The per-module `equivalence/`-style harness
   generalizes to one workspace that pulls each generated crate by path and runs
   its owning module's corpus; a red case is a portability finding, not a test
   nuisance.
4. **Signature contracts in the prompt.** The `Option<u64>` vs `Option<usize>`
   divergence shows prose alone under-specifies numeric types. Where a downstream
   caller depends on an exact type, the prompt (or IR) must state it — otherwise
   equivalence must stay behavioral and callers must tolerate the freedom.

## Scale-out — slice 1

The pilot proved one leaf. Slice 1 turns it into a production line: a
dependency-ordered module map (`docs/selfhost-map.md`), a shared equivalence
harness, and the next batch of wave-1 `napl-core` leaves generated by stage0.

### Modules generated this slice

Four more wave-1 pure leaves, each `napl gen rust --module <m>`, each **converged
on attempt 1 of 3**, fully attributed, locked, drift-clean:

| Module | Prompt approach | Attempts | Equivalence |
| --- | --- | ---: | --- |
| `extensions` | API named; six curated emoji aliases given by Unicode codepoint + name; ZWJ-composite and robot-alias negatives spelled out | 1/3 | **7/7** |
| `hash` | behavior + the known SHA-256 vector for `hello`; "use a well-established crate" (no algorithm pasted) | 1/3 | **4/4** |
| `parse_output` | fence-extraction behavior in prose, three given/expect cases | 1/3 | **3/3** |
| `text_diff` | LCS unified-diff + hunk parse/apply; `Hunk`/`HunkLine`/`HunkKind` public shape named so the corpus can read it; round-trip property stated | 1/3 | **11/11** |

Combined with the pilot, the shared harness runs **30/30** (`body_lines` 5,
`extensions` 7, `hash` 4, `parse_output` 3, `text_diff` 11).

### The shared harness

`selfhost/equivalence/` is now one cargo project that path-deps every generated
crate and carries **one test file per module** (`tests/<module>.rs`), each
replaying that module's exact hand-written `napl-core` unit corpus against the
generated code. `body_lines` migrated in (`tests/corpus.rs` → `tests/body_lines.rs`).
A red case is a portability finding, not a test nuisance.

### Multi-module layout — the friction found

The pilot was single-module and put `body_lines` **at the crate root** of the gen
target dir (`.napl/src/rust/`). That layout does **not** compose: the target dir
is fixed per target, generated files are locked `0444`, and a new module's gen
only unlocks files that module already owns — so a second module can never edit a
shared root `lib.rs` or workspace manifest to register itself.

The resolution that needs no toolchain change: new modules land as **nested
package crates** in subdirectories (`.napl/src/rust/<module>/`). Each gen creates
only its own subtree, touching nothing locked; `body_lines` stays untouched at the
root. Cargo silently ignores a nested package when invoked from the root, so the
tree needs no workspace manifest — and that same fact means the **in-gen
`cargo test` gate only exercises the root crate**, not the nested modules. The
shared equivalence harness is therefore the real cross-module gate. Roadmap: teach
the rust target to lay the target dir out as a Cargo workspace so every module is a
uniform member and the in-gen gate covers them all.

### Prompt under-specification — what the machine layer surfaced

The compile-as-dialogue model kept working: the `.mapl` files flag exactly what
prose left open rather than hiding divergence.

- **`extensions`**: return-type *ownership* was unstated — the agent returned
  `Vec<String>` from `prompt_extensions` but `Vec<&'static str>` from
  `machine_extensions`, and took `Option<&[&str]>` where the hand-written module
  takes `Option<&[String]>`. Behaviorally identical; the harness bridges the type
  difference. This is the same class of finding as the pilot's `Option<u64>` vs
  `Option<usize>`: prose under-specifies numeric/ownership contracts, and where a
  caller depends on the exact type the prompt must state it.
- **`parse_output`**: the machine layer flagged the "no closing backticks after a
  newline" wording as ambiguous. The generated code anchors fences to line starts
  (faithful to the prose "a line beginning with three backticks"), which is
  actually *stricter* than the hand-written `find("```")` — the two agree on every
  corpus case but would diverge on a mid-line ```` ``` ````. A latent portability
  finding the corpus does not yet pin.

### Escape-hatch list

Still **empty** — every wave-1 leaf attempted has converged on attempt 1.

### Recommended wave-2 batch

With all of wave 1's dependencies of these now self-hosted, the natural next batch
is the wave-2 leaves that depend only on generated wave-1 modules:
`blame` (→ `text_diff`), `schemas::lock` (→ `extensions`), and the pure
`schemas` leaves `line_range`/`frontmatter`/`ir`/`ordered_map` (still wave 1,
cheap corpora) to widen the base before `schemas::attribution`/`map`/`ml` in wave 2.

## Verdict

The loop is **viable for scaling** — and now demonstrated at breadth. Five
behavior-only prompts drove stage0 to generate Rust modules behaviorally
equivalent to their hand-written originals across 30 corpus cases, every one on the
first attempt, fully attributed, locked, and drift-clean. The remaining work is
more breadth (waves 2–3, then the `napl-cli` I/O phase) and two tractable toolchain
improvements the friction surfaced: a workspace layout for the rust target, and
signature/type contracts in the prompt where a caller depends on them — not a
missing capability in the loop itself.

## Scale-out — slice 2

Slice 2 does two things: it closes slice 1's layout gap by making the rust target a
**Cargo workspace**, and it finishes wave 1 by generating the remaining eight
`napl-core` leaves. Wave 1 is now fully self-hosted: **13/13 modules, 83
equivalence cases green**, escape-hatch list still empty.

### The workspace layout (toolchain change)

Slice 1's nested-crate layout composed only because Cargo ignores a nested package
from the root, which meant the in-gen `cargo test` gate covered only the root
crate — the nested modules escaped it. Slice 2 replaces that with a real workspace:

- The rust target adapter (`napl-core/src/targets.rs`) gained `workspace_layout:
  bool` and `attribution_exclude_root_files: Vec<String>`, plus a pure
  `workspace_manifest_toml(members)` renderer. The rust idiom guidance now tells
  the coding agent to write its module as a member crate in a subdirectory named
  after the module and to leave the workspace-root `Cargo.toml` alone.
- The gen loop (`napl-cli/src/cmd_gen.rs`) refreshes the toolchain-owned workspace
  root `Cargo.toml` before each module's attempts, listing every existing member
  crate directory plus the current module — so the member is registered *before*
  its `cargo test` runs (no orphan-workspace error) and the in-gen gate at the
  workspace root covers **every** member.
- **Ownership.** The root manifest is treated exactly like the guard files: written
  by the toolchain, excluded from attribution, not locked, not drift-checked,
  regenerated on every gen. To keep per-module `Cargo.toml` files attributed while
  excluding only the root one, the snapshot filter gained a root-only exclusion
  (`make_filter`'s new `exclude_root_files`, applied only at the walked tree's
  root). This is the clean split the campaign's attribution/locking rules require.
- **Conformance.** Scenario `74-gen-rust-target` was updated for the workspace
  layout (the fake agent writes `adder/src/lib.rs`; the toolchain writes the
  workspace root manifest; attribution maps `adder/src/lib.rs`). All 40 scenarios
  pass; the other 39 are byte-identical.

### The body_lines migration — the call made

`body_lines` was the one module that had to move (root crate → member crate); the
other four slice-1 modules were already in subdirectories. The choice was between a
mechanical move (rewrite map/attribution paths and hashes by hand) and a **clean
regen through the toolchain**. Regen won: it rebuilds map, attribution, IR, machine
layer, and journal from the toolchain's own mechanisms rather than hand-editing
hashes, and it re-proves the new layout on the "first" module. The only hand-work
was deletion (removing the old root-crate files and resetting `body_lines`' derived
state so gen treats it fresh) — no fabricated hashes. `body_lines` re-genned as a
member crate on attempt 1, `napl status` is clean for all modules, and the
workspace-root `cargo test` runs all members green.

### Modules generated this slice

Eight wave-1 leaves, each `napl gen rust --module <m>`, each **converged on attempt
1 of 3**, fully attributed, locked, drift-clean:

| Module | Prompt approach | Attempts | Equivalence |
| --- | --- | ---: | --- |
| `schemas::ordered_map` | insertion-order map; methods + serde-map round-trip; worked ordering examples | 1/3 | **4/4** |
| `schemas::line_range` | lenient deserializer (scalar / `[n]` / `[a,b]`), `>= 1` integer rule, integral-float accept | 1/3 | **8/8** |
| `schemas::ir` | serde types + `validate_ir` (non-empty module, object-or-string contracts, required function fields) | 1/3 | **6/6** |
| `schemas::frontmatter` | strict `---` delimiter split, field defaults, leading-blank-line body cleanup; `serde_yaml` values | 1/3 | **6/6** |
| `drift` | pinned guided-report text described line-by-line (indent, the two-space `current:  ` alignment, the three resolutions) | 1/3 | **3/3** |
| `guard` | pinned guard strings + the settings-merge state machine (Create/Update/Unchanged/Manual) with byte-exact snippet | 1/3 | **5/5** |
| `targets` | the target registry: three adapters, the workspace fields, `workspace_manifest_toml`, the unknown-target error text | 1/3 | **9/9** |
| `scanner` | the UTF-16 span model spelled out (BMP = 1 unit, astral = 2), the frontmatter/deps/refs scan, resolver precedence, every worked span | 1/3 | **12/12** |

The `drift`, `targets`, and `guard` equivalence tests include the byte-exact
user-facing strings their hand-written corpora pin (the drift report lines, the
adapter labels and unknown-target error, the settings snippet). The `schemas::*`
tests are serialize/deserialize round-trips over the exact hand-written vectors.

### Prompt under-specification — what surfaced

- **`scanner`** named its scan entry point `scan` where the hand-written module
  names it `scan_document`. Behaviorally identical; the harness bridges the naming
  difference with a `use scanner::scan as scan_document` alias — the same class of
  finding as the pilot's `Option<u64>`/`Option<usize>`: prose under-specifies the
  exact public name, and the equivalence gate stays behavioral. Every span value
  matched on the first attempt regardless.
- The machine layer kept flagging genuine open choices (error type names, ownership
  of returned strings) in the `.mapl` files rather than hiding divergence.

### Escape-hatch list

Still **empty** — every wave-1 leaf attempted (13/13) converged on attempt 1.

### Wave-2 readiness

Wave 1 is a complete, self-hosted base. The natural next batch is the wave-2 leaves
whose intra-crate deps are now all generated: `blame` (→ `text_diff`),
`schemas::lock` (→ `extensions`), `schemas::attribution`/`schemas::ml`
(→ `line_range`), `schemas::map` (→ `ordered_map`), and `reverse` (→ `body_lines`,
`schemas`). The workspace layout means each lands as one more member with the
in-gen gate already covering it; the only new wiring is intra-crate path-deps
between generated member crates when a wave-2 prompt depends on a wave-1 one.

### Verdict

The workspace change closed the one real structural gap slice 1 left open: the
in-gen `cargo test` now gates every module, not just the root. With that in place,
eight more behavior-only prompts — including the 634-LOC UTF-16 `scanner` and the
string-pinned `drift`/`targets`/`guard` — drove stage0 to behaviorally equivalent
Rust on the first attempt each, across 53 new corpus cases (83 total). Wave 1 self-
hosts end to end.

## Scale-out — slice 3 (wave 2)

Slice 3 generates all six **wave-2** `napl-core` modules — the ones that depend on
wave-1 crates. Waves 1–2 together now self-host: **19/19 modules, 155 equivalence
cases green**, escape-hatch list still empty, every wave-2 module converged on
**attempt 1 of 3**.

| Module | Builds on (generated crate) | Attempts | Equivalence |
| --- | --- | ---: | --- |
| `schemas::lock` | `extensions` | 1/3 | **20/20** |
| `schemas::attribution` | `schemas_line_range` | 1/3 | **9/9** |
| `schemas::ml` | `schemas_line_range` | 1/3 | **8/8** |
| `schemas::map` | `schemas_ordered_map` | 1/3 | **10/10** |
| `blame` | `text_diff` | 1/3 | **13/13** |
| `reverse` | `body_lines` + `schemas_attribution` + `schemas_line_range` | 1/3 | **12/12** |

(`schemas::lock` replays the hand-written 19-case corpus plus one added
empty-model rejection case, 20 total.)

### The new wiring — intra-workspace path-deps between generated crates

This is the first slice where a generated module composes on **another generated
module**, not just on external crates. The self-hosting claim depends on this being
real: a wave-2 crate must build on the *generated* wave-1 crate, not re-implement
its logic inline and not depend on hand-written `napl-core`.

The mechanism that emerged, using only existing toolchain machinery:

- **Prompt frontmatter `deps:`** names the NAPL-level dependency modules
  (`deps: [extensions]`, `deps: [schemas_line_range]`, …). The gen loop already
  surfaces this to the coding agent as a `Declared dependencies: …` line
  (`build_agent_task`), alongside a one-line summary of every other module in the
  project. No toolchain change was needed to read or route `deps:`.
- **Prompt prose** makes the requirement concrete and actionable. Each wave-2
  prompt carries a "Builds on the `<x>` module of this workspace" section that
  states, in words: the sibling member crate lives at `../<x>`, add a path
  dependency on it in your `Cargo.toml`, use its public API (named explicitly:
  `default_prompt_aliases`, `LineRange`, `OrderedMap`, `to_lines`/`parse_hunks`,
  `PromptBody`, `AttributionEntry`), do not reimplement it, and depend only on the
  generated sibling — never on a hand-written crate. No Rust or TOML is pasted; the
  requirement is prose, and the gen agent owns the resulting `Cargo.toml`.
- **The result**: every generated wave-2 crate's `Cargo.toml` carries the right
  path-dep (`extensions = { path = "../extensions" }`,
  `schemas_line_range = { path = "../schemas_line_range" }`, …) and its `src/lib.rs`
  calls the sibling's public items. `schemas_lock` calls
  `extensions::default_prompt_aliases`; `schemas_map` stores
  `schemas_ordered_map::OrderedMap`; `blame` parses hunks with
  `text_diff::{parse_hunks, to_lines, HunkKind}`; `reverse` path-deps three
  generated siblings at once. This is stage1 as a genuine composition.

The one friction the idiom guidance could have caused — its "Add no external
dependencies unless the described behavior genuinely requires one" line, and its
"leave every sibling module crate untouched" line — was pre-empted in prose:
each dep section states that the sibling path-dep is *part of the same workspace,
not an external crates.io dependency*, and depending on a sibling by path only
reads it (never edits it), which the "leave siblings untouched" rule permits. With
that phrasing, **no toolchain change was required**: `targets.rs` idiom text is
untouched, the 40-scenario corpus is byte-identical, and scenario 74's goldens did
not need regenerating (the rust adapter's layout output is unchanged).

### The equivalence harness at cross-module scale

Each wave-2 module's harness file replays that module's exact hand-written
`napl-core` corpus against the generated crate, and — the new part — **constructs
its inputs from the generated sibling crates' types**. `reverse`'s test builds
`schemas_attribution::AttributionEntry` values out of `schemas_line_range::LineRange`
values and feeds them to the generated `reverse`; `blame`'s test builds patches
with `text_diff::unified_diff`; `schemas_map`'s round-trips through the generated
`OrderedMap` serde. Because path-deps to the same member crate unify, the
`LineRange` a `reverse` match carries *is* the `LineRange` its `AttributionEntry`
was built from — the composition is type-real, not just behavior-real. Divergences
are bridged behaviorally as before: each generated crate surfaces its **own** error
type (`LockError`, `AttributionError`, `MlError`, a `String` parse error) where the
hand-written module shares one `SchemaError`; equivalence compares accept/reject and
resolved values, never the error type.

### The `blame`/`journal` call

`blame` was parked in the map as "→ `text_diff`, journal types". Reading the
hand-written source settled it: `blame` depends **only** on `text_diff`
(`parse_hunks`, `to_lines`, `HunkKind`, and `unified_diff` in its tests). Its
`BlameSourceEntry` is a blame-local struct, not a `schemas::journal` type — the
dependency runs the other way (`schemas::journal` → `blame`, a wave-3 edge). So no
journal pull-forward was needed; `blame` generated cleanly against `text_diff`
alone.

### Escape-hatch list

Still **empty** — 19/19 modules across waves 1–2 converged on attempt 1.

### Wave-3 readiness

Waves 1–2 are a complete, self-hosted base, and every wave-3 module's intra-crate
deps are now generated: `schemas::journal` (→ `blame`, `text_diff`), `prompts`
(→ `schemas`, `targets`), `yaml` (→ `schemas`), `incremental` (→ `schemas`). The
path-dep-in-prose idiom proven here carries directly to them; `prompts` and `yaml`
are the larger corpora and the first real test of composing over the whole
`schemas` surface at once.

### Verdict

Wave 2 self-hosts end to end. Six behavior-only prompts — including the 553-LOC
`schemas::map` mutation engine and the three-sibling `reverse` — drove stage0 to
behaviorally equivalent Rust on the first attempt each, across 72 new corpus cases
(155 total). The decisive new evidence is compositional: the generated modules
build on the *generated* wave-1 crates by path, expressed entirely in prompt prose,
with no toolchain change and the conformance corpus byte-identical.

## Scale-out — slice 4 (wave 3): `napl-core` FULLY SELF-HOSTS

Slice 4 generates the four **wave-3** `napl-core` modules — the aggregates that
compose over waves 1–2 — and with them **phase 1 is complete**. The entire pure
crate now regenerates from prose: **23/23 modules, 189 equivalence cases green,
escape-hatch list still empty, every module converged on attempt 1 of 3.**

| Module | Builds on (generated crate) | Attempts | Equivalence |
| --- | --- | ---: | --- |
| `schemas::journal` | `blame` + `text_diff` | 1/3 | **8/8** |
| `incremental` | `schemas_attribution` + `schemas_line_range` | 1/3 | **3/3** (2 corpus + 1 composition) |
| `yaml` | `schemas_attribution` + `schemas_ir` + `schemas_ml` + `schemas_line_range` | 1/3 | **9/9** |
| `prompts` | `schemas_attribution` + `schemas_frontmatter` + `schemas_line_range` + `targets` | 1/3 | **14/14** (7 corpus + 7 byte-exact pins) |

The path-dep-in-prose idiom from slice 3 carried over verbatim: each wave-3 prompt
names its sibling member crates (`../blame`, `../text_diff`,
`../schemas_attribution`, `../targets`, …), states that the path-dep is a workspace
sibling and not a crates.io dependency, and forbids reimplementing the sibling or
depending on any hand-written crate. Every generated `Cargo.toml` carries the right
path-deps and each `src/lib.rs` calls the sibling's public API —
`schemas_journal` builds patches with `text_diff::unified_diff` and yields
`blame::BlameSourceEntry`; `yaml` reads `schemas_ir::Ir`/`schemas_ml::Ml`/
`schemas_attribution::Attribution` fields; `prompts` reads `targets::TargetAdapter`
and `schemas_frontmatter::Frontmatter`. No toolchain change was required and the
40-scenario conformance corpus stays byte-identical.

### Byte-exact where the toolchain pins bytes

Two wave-3 modules produce **byte-load-bearing** output, and their prompts pin it
as output data (never as pasted source — the same discipline `drift`/`guard`/
`targets` used for their user-facing strings):

- **`yaml`** is a focused emitter matching `eemeli/yaml`'s block style; the corpus
  pins its scalar-styling and whole-document bytes. The prompt describes the
  plain/single/double scalar heuristics and the block-emission rules in prose and
  gives the exact document goldens; the generated emitter reproduces every pinned
  byte (attribution/IR/machine-layer documents included).
- **`prompts`** builds the coding-agent tasks and the IR/attribution/machine-layer
  derivation prompts. Its four system-prompt constants are load-bearing — the
  conformance corpus asserts substrings and the fake backend routes on the
  `intermediate representation` / `MACHINE LAYER` markers — so the prompt supplies
  them verbatim inside four-backtick fences (to preserve the literal triple-backtick
  ```` ```yaml ```` sequences) and describes each builder's line-by-line assembly.
  The generated constants came out **byte-identical** to the hand-written ones, and
  the equivalence gate pins them so — plus full byte-exact assertions on the six
  adapter-independent builders — to guarantee stage1 swap-in safety beyond the
  substring corpus.

### `statusclass` / `classify` — left in their I/O phases

Two pure-looking classifiers were weighed for pull-forward and **declined**: both
`napl-cli/src/statusclass.rs` and `napl-lsp/src/classify.rs` drag I/O — their
`detect_drift` reads generated files off disk — so the module as written is not
behaviorally-unit self-hostable. `statusclass`'s two unit tests are pure render
(`StatusEntry::line`, `is_error`); `classify` has no unit tests. They stay in
phases 2 and 3; the map records the split-first path by which the pure render slice
could be pulled forward later.

### Escape-hatch list

Still **empty** — 23/23 modules across all three waves converged on attempt 1.

### Phase 1 complete — the stage1 swap-in plan

`napl-core` now has a complete generated twin: every module of the hand-written
pure crate has a behaviorally-equivalent generated crate under
`selfhost/.napl/src/rust/`, each gated by that module's exact unit corpus in the
shared harness. **Stage1** is the toolchain built with those generated crates in
place of the hand-written `napl-core` modules — same public API, corpus as gate.
The plan:

1. **Assemble a stage1 `napl-core`.** Behind the crate's existing public surface
   (`lib.rs` re-exports), route each module to its generated crate. Two shapes:
   the mechanical one is to make `napl-core` depend on the generated member crates
   by path and re-export their items under the current module paths (`pub use
   yaml_gen as yaml`, etc.); the cleaner long-run one is to promote the generated
   `.napl/src/rust/` workspace to *be* `napl-core`'s modules. Start mechanical —
   it is reversible and keeps the diff auditable.
2. **Reconcile the API seams the equivalence gate already mapped.** The harness
   documented every behavioral-but-not-type-identical divergence: each generated
   schema crate surfaces its **own** error enum (`FrontmatterError`,
   `AttributionError`, `MlError`, `IrValidationError`, a `String` journal-parse
   error) where hand-written `napl-core` shares one `SchemaError`; a few names and
   numeric/ownership types differ (`scan` vs `scan_document`, `Option<u64>` vs
   `Option<usize>`). Stage1 needs a thin adapter layer at the `napl-cli`/`napl-lsp`
   boundary that maps these to the callers' expected shapes — or those errors get
   unified in the prompts. This adapter is the real remaining engineering, and the
   equivalence tests are its spec.
3. **Gate stage1 on the full corpus, then on conformance.** Stage1 is accepted only
   when (a) every module's unit corpus passes against the swapped-in code — already
   true, that is what the harness proves — and (b) the 40-scenario conformance suite
   stays byte-identical with the stage1 binary driving it. Conformance is the CLI's
   observable contract; passing it byte-for-byte with generated `napl-core` inside
   is the phase-1 fixpoint demonstrated end to end.
4. **Then phases 2–3.** With a stage1 core proven, the I/O crates (`napl-cli`
   `cmd_*`/`fsutil`/`process`, `napl-lsp` server) become the next frontier, gated by
   conformance and the LSP integration suite rather than unit vectors. `cmd_gen`
   self-hosting is the true fixpoint — the generator regenerating itself.

### Verdict

Phase 1 is done. Four more behavior-only prompts — including the byte-exact
`eemeli/yaml` emitter and the 523-LOC `prompts` builder whose four system-prompt
constants came out byte-identical — drove stage0 to behaviorally equivalent Rust on
the first attempt each, across 34 new corpus cases (189 total). Every module of the
pure `napl-core` crate now self-hosts, composed on generated siblings by path, with
no toolchain change and conformance byte-identical. What remains is the stage1
swap-in (a bounded adapter layer the equivalence gate already specifies) and the
I/O phases.

## Stage1 swap-in — DONE (the shipping binary now runs generated `napl-core`)

**The shipping `napl` binary now executes the NAPL-generated `napl-core` modules,
not the hand-written ones**, gated byte-identical against the stage0 binary. The
hand-written module bodies are deleted; each `napl-core` module is a thin adapter
over its generated crate. **151 adapter LOC replaced 3,719 LOC of hand-written
implementation** (the hand-written unit corpora stay in place as the in-crate
regression net — 181 `napl-core` unit tests now run against the adapters).

### Workspace-membership call

The generated crates were **left as members of the `selfhost/.napl/src/rust/`
workspace and pulled in as plain cross-workspace path-deps** from
`napl-core/Cargo.toml` (e.g. `path = "../../../selfhost/.napl/src/rust/blame"`).
They were **not** added to the `rust/` cargo workspace. This is the cleanest split
given the campaign's rules:

- `selfhost/` stays byte-untouched — the generated tree keeps its own virtual
  `[workspace]` manifest, its `0444` locks, and its drift guard; nothing there was
  edited (verified: `napl status` in `selfhost/` is clean, 23/23 modules).
- Because the generated crates are dependencies and not workspace members,
  `cargo clippy --workspace` lints only the three hand-written crates plus the
  adapters — it does **not** lint the locked generated code (correct: it is not
  ours to lint).
- The 14 generated crate names that collide with a `napl-core` top-level module
  name (`blame`, `body_lines`, `drift`, `extensions`, `guard`, `hash`,
  `incremental`, `parse_output`, `prompts`, `reverse`, `scanner`, `targets`,
  `text_diff`, `yaml`) are pulled in under a `gen_*` package alias to avoid the
  module-vs-extern-crate name clash; the 9 `schemas_*` crates need no alias.

`napl-cli` and `napl-lsp` required **zero edits** — `napl-core`'s public surface
(the `lib.rs` module tree, every re-exported type, every function signature, and
the shared `SchemaError`) is preserved exactly. That is the proof this is a true,
reversible swap.

### Adapter seam catalog

Every divergence the equivalence harness mapped, and how the adapter bridges it:

| Module | Seam | Bridge |
| --- | --- | --- |
| `scanner` | generated entry point is `scan`; callers expect `scan_document` | `pub use gen_scanner::scan as scan_document` |
| `extensions` | generated `CURATED_PROMPT_ALIASES`; callers use `DEFAULT_PROMPT_ALIASES` | re-export under the old name |
| `extensions` | `prompt_extensions`/`is_prompt_file` take `Option<&[&str]>` (gen) vs `Option<&[String]>` (callers) | wrapper collects `&[String]` → `Vec<&str>` |
| `extensions` | `machine_extensions` returns `Vec<&'static str>` (gen) vs `Vec<String>` | wrapper maps to owned `String`s |
| `schemas::attribution` | own `AttributionError` | `map_err` → `SchemaError::Deserialize` |
| `schemas::ir` | own `IrValidationError` | `map_err` → `SchemaError::Deserialize` |
| `schemas::ml` | own `MlError` | `map_err` → `SchemaError::Deserialize` |
| `schemas::lock` | own `LockError` | `map_err` → `SchemaError::Deserialize` |
| `schemas::map` | parse error is a `String` | `map_err(SchemaError::Deserialize)` |
| `schemas::frontmatter` | own `FrontmatterError` **and** reworded Display text | per-variant map to the exact `SchemaError` message text the CLI contract pins (see escape-hatch discussion) |

The consumers (`napl-cli/src/error.rs`, `napl-lsp/src/classify.rs`) destructure
`SchemaError::Deserialize(m) | SchemaError::Validation(m) => m`, using only the
inner message string, so collapsing all generated schema errors onto
`SchemaError::Deserialize` is observably identical. The much-advertised
`Option<u64>` vs `Option<usize>` `body_lines` seam **no longer exists** — the
re-genned `body_lines` crate already returns `Option<usize>`; that module is a
straight re-export.

Test-scope-only imports the re-export adapters can't provide (types the original
module bodies imported and the kept unit tests reach through `use super::*`) are
re-added under `#[cfg(test)]`: `reverse` (`AttributionEntry`, `LineRange`),
`schemas::attribution`/`schemas::ml` (`LineRange`), `schemas::lock`
(`default_prompt_aliases`), and `scanner`'s test-local `pos`/`span` helpers.

### Per-module swap status

**22 of 23 modules run generated code; 1 is on the escape-hatch list.**

Swapped in (generated crate behind the adapter): `blame`, `body_lines`, `drift`,
`extensions`, `guard`, `hash`, `incremental`, `parse_output`, `prompts`,
`reverse`, `scanner`, `targets`, `text_diff`, `yaml`, and the schemas
`attribution`, `frontmatter`, `ir`, `line_range`, `lock`, `map`, `ml`,
`ordered_map`.

`schemas::frontmatter` is swapped **with an error-message seam bridge** rather
than escape-hatched, because escape-hatching it would cascade: the generated
`prompts` crate composes on `schemas_frontmatter::Frontmatter`, so reverting
`napl-core`'s canonical `Frontmatter` to a hand-written type would create a type
schism at `build_agent_task`. The bridge maps each `FrontmatterError` variant to
the hand-written message text (e.g. `InvalidYaml(e)` →
`"invalid YAML frontmatter: {e}"`) that conformance scenario
`50-frontmatter-invalid` pins (`napl: invalid YAML frontmatter`).

### Escape-hatch list

- **`schemas::journal`** — the generated `read_journal_str` emits corrupt-line
  **warning strings** (`"line 2: expected ident …"` / `"line N: journal entry
  failed validation"`) that differ from the hand-written / CLI-contract format
  (`"skipping corrupt line 2 (invalid JSON)"`), which conformance scenario
  `34-journal-corrupt-line` pins byte-for-byte. The equivalence gate only compares
  `(entries, warnings.len())`, never the warning text, so this divergence lives
  **outside the adapter spec** — a genuine observable behavioral gap, not a seam.
  Per the escape-hatch rule the hand-written body was restored; the generated
  `schemas_journal` crate stays fully equivalence-green (8/8) and drift-clean,
  simply not wired into the shipping binary. Reconstructing the warning text in the
  adapter would mean re-parsing and re-classifying each line — reimplementation,
  not conversion — and `schemas_journal` has no generated dependents, so the
  escape-hatch is clean and cascades nowhere. Exact failing case: a journal with a
  non-JSON line 2, expected stdout `journal: skipping corrupt line 2 (invalid
  JSON)`, generated actual `line 2: expected ident at line 1 column 2`.

### Gate results

1. `cargo test --workspace` — **220 pass, 0 fail** (181 `napl-core` unit +
   18 `napl-cli` + 16 `napl-lsp` + 5 `cross_check` integration), the hand-written
   corpora now running against the adapters.
2. `cargo clippy --workspace --all-targets` — **clean** (adapters only; generated
   crates are not workspace members and are not linted).
3. `cargo build --release` — green; the `napl` symlink already targets it.
4. **Conformance — 40/40 BYTE-IDENTICAL** with the stage0 binary. This is the
   phase-1 fixpoint gate, met end to end.
5. `selfhost/` — `napl status` clean, 23/23 modules; the generated tree is
   byte-untouched.
6. Equivalence harness — **189/189**, untouched.

Sanity: `napl --version` → `0.1.0`; `napl init` + fresh `napl status` in a temp
dir behave identically.

### Verdict — phase 1 fixpoint reached

The observable contract of the CLI is reproduced **byte-for-byte by a binary whose
pure core is generated from prose**, with 22 of 23 `napl-core` modules running
generated code and a single, honestly-recorded escape-hatch (`schemas::journal`,
a warning-text divergence the equivalence gate never claimed to cover). The swap is
mechanical and reversible: `napl-cli`/`napl-lsp` are unedited, the hand-written
unit corpora ride along as the regression net, and reverting is deleting the
adapters. The stage1 core is proven; the I/O crates (`cmd_*`, the LSP server) are
the next frontier.

## Journal escape-hatch cleared + Phase 2 opened

This slice finishes stage1 (the escape-hatch list goes to **zero**) and opens
phase 2 (the first batch of `napl-cli` pure cores self-hosts). Summary of the new
state: **28/28 generated modules, 200 equivalence cases green, conformance 40/40
byte-identical, escape-hatch list empty.**

### JOB A — the `schemas::journal` escape-hatch, cleared

The one stage1 escape-hatch was a **warning-text divergence**: the generated
`read_journal_str` emitted `"line 2: expected ident …"` where the CLI contract and
conformance scenario `34-journal-corrupt-line` pin `journal: skipping corrupt line
2 (invalid JSON)` byte-for-byte. The equivalence gate had only ever compared
`(entries, warnings.len())`, so the text was never pinned and the divergence lived
outside the adapter spec.

The fix was **prompt, not adapter** — the honest way off the hatch:

- `selfhost/schemas_journal.napl` now pins the exact warning as **behavior prose**:
  a two-phase parse (first try the line as arbitrary JSON — a syntax failure yields
  exactly `journal: skipping corrupt line {n} (invalid JSON)`; only a line that
  *is* valid JSON but fails to deserialize/validate takes the
  `journal: skipping corrupt line {n} ({reason})` branch with the same prefix), and
  a byte-exact `given`/`expect` frontmatter case pinning the line-2 warning. The
  prose explicitly forbids collapsing the two phases into one deserialize attempt
  (which is what made a raw non-JSON line surface a serde parse message before).
- **Regen:** `napl gen rust --module schemas_journal` converged on **attempt 1/3**.
  The regenerated crate now does the generic-JSON syntax check first and formats the
  pinned bytes.
- **Equivalence extended to assert TEXT, not just count:** `tests/schemas_journal.rs`
  now asserts `warnings[0] == "journal: skipping corrupt line 2 (invalid JSON)"`
  and adds a dedicated conformance-mirroring case — **9/9** (was 8/8).
- **Swapped in:** `napl-core`'s hand-written `schemas::journal` body is deleted and
  replaced by the same thin re-export the other 22 modules use
  (`pub use schemas_journal::{…}`), with `schemas_journal` added as a path-dep. The
  much-discussed error-message seam **no longer exists** — there is no bridge, just
  a re-export. The stage1 escape-hatch list is now **empty**; all 23 `napl-core`
  modules ship generated code. Conformance scenario `34-journal-corrupt-line` passes
  byte-identical with the generated reader driving it.

### JOB B — phase 2 survey + first batch (`napl-cli` self-host)

**Survey.** `napl-cli`'s I/O modules were classified into a pure core (a
deterministic function the equivalence harness can gate) and a thin I/O shell
(filesystem/subprocess plumbing that stays hand-written). The full per-module split
is in `docs/selfhost-map.md` → "Phase 2". The rule mirrors stage1:
**extract-pure-core, keep-thin-I/O-shell**, conformance byte-identical before and
after each swap, and the conformance corpus is the behavioral spec for the `cmd_*`
handlers while the pure cores are gated by their existing unit corpora.

**First batch — five low-risk leaves, all converged on attempt 1/3, all swapped in:**

| Generated crate | Replaces (pure slice) | Deps | Equivalence |
| --- | --- | --- | --- |
| `clock_fmt` | `clock::iso_from_millis` | — | 3/3 (byte-exact ISO) |
| `paths_core` | `paths::{resolve_paths, NaplPaths, rel_to}` | — | 2/2 (rel_to + full layout) |
| `statusclass_render` | `statusclass::{FileStatus, StatusEntry, line, is_error}` | — | 2/2 (byte-exact status lines) |
| `driftdetect_replay` | `driftdetect::reconstruct_file_content` | `schemas_journal`, `text_diff` | 2/2 |
| `snapshot_diff` | `snapshot::diff_snapshots` | — | 1/1 |

No extraction refactor was required: each pure core was already a cleanly separable
function/type inside its module (the shells already called them), so each swap is a
`pub use <generated_crate>::…` behind the unchanged call sites with the hand-written
pure body deleted, and the module's unit corpus kept as the regression net.

**The decisive new evidence is `driftdetect_replay`** — the first **phase-2** pure
core to compose on **generated phase-1** crates. It path-deps the generated
`schemas_journal` (for `JournalEntry`) and `text_diff` (for `parse_hunks`/
`apply_hunks`), and because JOB A made `napl-core::schemas::JournalEntry` a re-export
of `schemas_journal::JournalEntry` and both crates path-dep the *same*
`schemas_journal`, the types unify: the `driftdetect` shell passes
`&[napl_core::schemas::JournalEntry]` straight into the generated
`reconstruct_file_content`. Phase 2 composes on phase 1 with no glue.

**Modules kept as shells (documented, not genned):** `fsutil` (every fn is fs I/O;
only the mode constants are pure, no pure unit test), `error` (a type + `From` trait
glue over the hand-written `SchemaError`/`io::Error` — inseparable from caller
types), `process`/`state`/`main`, and the `cmd_*` handlers (conformance-gated
orchestration, `cmd_gen` being the eventual fixpoint). No speculative I/O wrappers
were genned. The `snapshot`/`SnapshotFilter` exclusion predicate is pure but has no
direct unit test, so it is parked for batch 2 (add the test first).

### Gate results

1. `cargo test --workspace` — **220 pass, 0 fail** (181 `napl-core` unit + 18
   `napl-cli` + 16 `napl-lsp` + 5 `cross_check`), the hand-written corpora running
   against the adapters and the phase-2 shells.
2. `cargo clippy --workspace --all-targets` — **clean**.
3. `cargo build --release` — green.
4. **Conformance — 40/40 BYTE-IDENTICAL**, including `34-journal-corrupt-line` now
   driven by the generated journal reader and every `napl-cli` scenario driven by
   the phase-2 shells over generated pure cores.
5. `selfhost/` — `napl status` clean, **28/28 modules** (23 phase-1 + 5 phase-2),
   the generated tree drift-clean.
6. Equivalence harness — **200/200** (189 phase-1 + 1 new journal warning-text +
   10 phase-2 batch-1).

### Verdict

Stage1 is complete with an empty escape-hatch: all 23 `napl-core` modules ship
generated code, the journal warning-text gap closed by pinning the bytes in the
prompt and re-genning (not by an adapter hack). Phase 2 is open and self-hosting:
five `napl-cli` pure cores are generated from prose and swapped in behind their I/O
shells, conformance byte-identical, one of them composing on generated phase-1
crates. What remains in phase 2 is more pure-core extraction (batch 2+) and,
ultimately, the conformance-gated `cmd_gen` fixpoint — the generator regenerating
itself.

## Phase 2 — batch 2 (`napl-cli` derivation/render slices)

Batch 2 continues shrinking `napl-cli`'s hand-written surface: four more pure
cores extracted, generated from prose, and swapped in behind their I/O shells,
each converged on **attempt 1 of 3**. New totals: **32/32 generated modules, 214
equivalence cases green, conformance 40/40 byte-identical, escape-hatch list
empty.**

| Generated crate | Replaces (napl-cli pure slice) | Deps | Equivalence |
| --- | --- | --- | --- |
| `snapshot_filter` | `snapshot::{SnapshotFilter, make_filter, is_excluded_dir, is_excluded_file}` | — | 1/1 |
| `blame_render` | `cmd_blame::{mode_str, format_blame_row, why_line, render_blame_gen}` | `blame`, `schemas_journal` | 7/7 |
| `watch_filter` | `cmd_watch::{is_ignored, IGNORED_DIRS}` | — | 2/2 |
| `reconcile_derive` | `cmd_reconcile::{editable_drifted, build_reconcile_files}` | `drift`, `prompts`, `text_diff` | 4/4 |

### Corpus-first + extraction-first — the new discipline vs. batch 1

Batch 1's five slices were already cleanly separable functions with existing unit
tests, so each swap was a bare re-export. Batch 2 was harder: three of the four
modules had **no** unit tests and their pure logic was inlined in an I/O function.
Each therefore went through the full campaign discipline:

- **`snapshot_filter`** — `SnapshotFilter`/`make_filter` were pure but only tested
  indirectly through the fs walk. A **direct** filter unit test
  (`filter_predicate_decides_dirs_files_roots_and_suffixes`) was added first, and
  the private `is_excluded_file` was made `pub` alongside a new `is_excluded_dir`
  (the walk now calls `filter.is_excluded_dir(name)` instead of reaching into the
  field). Conformance stayed byte-identical, then the crate genned and swapped.
- **`blame_render`** — `cmd_blame` had no tests and its single-gen summary was
  printed by an inline `blame_gen` that interleaved `println!`s. It was refactored
  into a pure `render_blame_gen(entries, gen) -> BlameGenRender { text, exit_code }`
  that returns the exact multi-line block as one joined string (the shell prints it
  with a single `println!`, byte-identical to the old line-by-line printing), plus a
  7-case corpus over `mode_str`/`format_blame_row`/`why_line`/`render_blame_gen`. It
  composes on generated `blame` (`BlameLine`, `first_prompt_diff_line`) and
  `schemas_journal` (`JournalEntry`, `JournalMode`) by path.
- **`watch_filter`** — extracted the `is_ignored` path predicate and its
  `IGNORED_DIRS` constant with a 2-case corpus (toolchain/VCS dirs ignored at any
  depth; ordinary prompt paths kept — the `.napl` file **extension** is not a
  `.napl` directory component).
- **`reconcile_derive`** — extracted the two derivations inlined in the reconcile
  loop: `editable_drifted` (keep only hand-edited files that still have on-disk
  content, in order) and `build_reconcile_files` (one `ReconcileFile` per editable
  file, using its recorded diff or a `unified_diff("", current)` empty-baseline
  fallback), with a 4-case corpus. It composes on generated `drift`, `prompts`, and
  `text_diff` by path.

Every corpus addition was test-only (conformance byte-identical), every extraction
refactor kept conformance byte-identical **before** the swap, and every swap kept it
byte-identical again.

### More phase-2-on-phase-1 composition

Like batch 1's `driftdetect_replay`, two of batch 2's cores compose on generated
phase-1 crates and the types unify with no glue: `blame_render` takes
`&[napl_core::schemas::JournalEntry]` (a re-export of `schemas_journal::JournalEntry`)
and `&napl_core::blame::BlameLine` straight from the shell; `reconcile_derive` takes
`&[napl_core::drift::DriftedFile]` and returns `napl_core::prompts::ReconcileFile`s.
Because napl-core re-exports the generated types and every crate path-deps the same
sibling, the shell passes its already-read data through unchanged.

### `error` declined again

`error` was re-weighed and left a shell: its only pure logic is the
`From<SchemaError>` message extraction, trait glue inseparable from the hand-written
`SchemaError`/`io::Error` caller types — no separable pure core with a behavioral
corpus. `cmd_status`/`cmd_init` similarly have no separable untested pure slice.

### Gate results

1. `cargo test --workspace` — **234 pass, 0 fail** (181 `napl-core` unit + 5
   `cross_check` + 32 `napl-cli` — up 14 with the new batch-2 unit corpora — + 16
   `napl-lsp`), the hand-written corpora running against the adapters and phase-2
   shells.
2. `cargo clippy --workspace --all-targets` — **clean** (adapters + shells; the
   generated crates are not workspace members and are not linted).
3. `cargo build --release` — green.
4. **Conformance — 40/40 BYTE-IDENTICAL**, including every `blame`/`watch`/
   `reconcile`/`snapshot`-touching scenario driven by the phase-2 shells over the new
   generated pure cores.
5. `selfhost/` — `napl status` clean, **32/32 modules** (23 phase-1 + 5 phase-2
   batch-1 + 4 phase-2 batch-2), the generated tree drift-clean.
6. Equivalence harness — **214/214** (200 prior + 14 batch-2: 1 + 7 + 2 + 4).

### No stage1-binary misbehavior

All four gens ran through the shipping stage1+2 binary (generated code generating
code) and converged on attempt 1 with valid attribution, machine layer, journal, and
`0444` locks — no fixpoint regression observed.

### Verdict

Batch 2 shrinks `napl-cli`'s hand-written surface by four more pure cores — the
byte-exact `blame` renderer, the `watch` ignore predicate, the `reconcile` input
derivation, and the `snapshot` exclusion filter — all generated from prose and
swapped in behind unchanged I/O shells, conformance byte-identical, three of them
requiring a corpus and an extraction refactor first and two composing on generated
phase-1 crates. What remains in phase 2 is the scarcer pure slices outside `cmd_gen`
and, ultimately, the conformance-gated `cmd_gen` fixpoint — the generator
regenerating itself.

## Phase 2 — batch 3 (the `cmd_gen` decomposition — the fixpoint push)

Batch 3 opens the generator itself. `cmd_gen.rs` (1,133 LOC) is the stage0
orchestrator — the last thing to self-host and the true fixpoint when it does. It
cannot be self-hosted whole (it spawns processes, drives the LLM, and writes the
journal), but its **pure decision logic** can be sliced out, generated from prose,
and swapped back in. Batch 3 does exactly that for four slices, each converged on
**attempt 1 of 3**. New totals: **36/36 generated modules, 225 equivalence cases
green, conformance 40/40 byte-identical, escape-hatch list empty.**

`docs/selfhost-map.md` → "Batch 3 — the `cmd_gen` decomposition" carries the full
function-by-function survey (every `cmd_gen.rs` function classified as a pure slice
or irreducible I/O shell). The four slices generated:

| Generated crate | Replaces (cmd_gen pure slice) | Deps | Equivalence |
| --- | --- | --- | --- |
| `gen_classify` | `is_source_file` (+ the source-extension set), `first_meaningful_line`, `split_body_lines` | — | 3/3 |
| `gen_prompt_diff` | `compute_prompt_diff` | `incremental` | 2/2 |
| `gen_attribution_check` | `assert_attribution_sane` (byte-exact error strings) | `schemas_attribution` | 4/4 |
| `gen_mode` | `can_incremental` + the byte-exact `mode:` message renderers | — | 2/2 |

### Corpus-first — `cmd_gen` had no unit tests

`cmd_gen` shipped with **no** `#[cfg(test)] mod tests`. The batch therefore added an
11-case unit corpus to the hand-written module first (test-only, conformance
byte-identical), pinning the exact behavior of each slice — including byte-exact
assertions on the four `assert_attribution_sane` error/ok cases and the four
`mode:` status lines the conformance corpus substring-checks. That corpus is both
the extraction safety net and the equivalence spec replayed against the generated
crates; it rides along in `cmd_gen.rs` as the regression net after the swaps.

### One extraction refactor, three bare re-exports

Three slices (`is_source_file`/`first_meaningful_line`/`split_body_lines`,
`compute_prompt_diff`, `assert_attribution_sane`) were already cleanly separable
functions, so each swap is a `use <crate>::…` behind unchanged call sites with the
hand-written body deleted. Only **`gen_mode`** needed an extraction refactor first:
the full-vs-incremental decision (`can_incremental`) and the `mode:` status lines
were inlined in `build_task_builder`. They were lifted into a pure predicate over
four primitive booleans plus three message renderers (a `FullModeReason` enum + an
`incremental_mode_message(changed, affected)` renderer), the shell rewritten to
compute the booleans and print the rendered lines — conformance byte-identical
**before** the gen, and again after the swap.

### More phase-2-on-phase-1 composition, and one by-value seam

`gen_prompt_diff` composes on generated `incremental` (`diff_body_lines(...).unified`)
and `gen_attribution_check` on generated `schemas_attribution`
(`Attribution`/`AttributionEntry`). Because napl-core re-exports those generated
types and every crate path-deps the same sibling, the shell passes
`napl_core::schemas::Attribution` straight into the generated checker and the types
unify with no glue — the same evidence batches 1–2 produced, now inside the
generator. The one seam: the generated `gen_mode::full_mode_message` takes
`FullModeReason` **by value** where the hand-written helper took it by reference —
bridged at the three call sites (pass by value). The equivalence is behavioral
(identical strings); the by-value/by-ref difference is exactly the kind of
under-specified ownership contract the campaign bridges at the boundary.

### `state.rs` — surveyed, declined honestly

`state.rs` (89 LOC) was surveyed for a pure slice. Every reader/writer is fs I/O
wrapping a pure core that is **already generated** (`parse_map`, `read_journal_str`,
`parse_lock`, `resolve_prompt_aliases`, `next_gen_number` are phase-1 `napl-core`
modules). `default_lock()` is pure but a literal constructor of an `HlLock` from
schema constants — schema glue with no derivation, no branching, and no independent
behavioral corpus, the same class as `error` (declined in batches 1–2). No
corpus-worthy pure slice; `state.rs` stays a shell.

### Gate results

1. `cargo test --workspace` — **245 pass, 0 fail** (181 `napl-core` unit + 5
   `cross_check` + 43 `napl-cli` — up 11 with the new `cmd_gen` corpus — + 16
   `napl-lsp`), the hand-written corpora running against the adapters and shells.
2. `cargo clippy --workspace --all-targets` — **clean** (0 warnings).
3. `cargo build --release` — green.
4. **Conformance — 40/40 BYTE-IDENTICAL** at every step: after the `gen_mode`
   extraction refactor, and after each of the four swaps.
5. `selfhost/` — `napl status` clean, **36/36 modules** (32 prior + 4 batch-3),
   the generated tree drift-clean.
6. Equivalence harness — **225/225** (214 prior + 11 batch-3: 3 + 2 + 4 + 2).

### How far can the shell shrink? — the fixpoint, honestly

Batch 3 took `cmd_gen.rs` from 1,133 LOC to a **~1,088-LOC hand-written shell**
(plus a 137-LOC regression corpus). The extracted ~45 LOC were the generator's
*pure decision logic* — file classification, description derivation, prompt-diff
derivation, the attribution sanity gate, and the full-vs-incremental mode
selection. What remains is **genuinely irreducible I/O**: spawning the coding agent
and the test command (`run_attempts`, `retry_for_change`), the LLM derivation loops
for IR/attribution/machine-layer (`derive_ir`, `derive_attribution_gated`,
`derive_ml` — each an LLM round-trip, not a pure function), the filesystem reads
and writes (numbered files, prior body/attribution, journal append, guard files,
workspace manifest), and the top-level `run_gen_locked` orchestrator that sequences
them.

This clarifies what "**cmd_gen regenerates itself**" can mean. The generator will
never be a single pure function the equivalence harness replays — its essence is
orchestrating a filesystem, subprocesses, and an LLM. The honest fixpoint is:
**every pure decision the generator makes is generated code, and the thin I/O shell
that sequences those decisions is the only hand-written part left** — and the proof
is that the shipping binary, whose generator's pure logic is now generated from
prose, still drives the whole 40-scenario conformance corpus byte-identically,
including the `mode:` lines and attribution accounting that batch 3's slices now
render. The shell can shrink further (the `enforce_no_op` decision, the
workspace-member merge, the numbered-files gate), but it converges on the I/O
skeleton, not on zero — and that skeleton, gated by conformance, is the fixpoint.

### Verdict

Batch 3 slices the generator's own pure decision logic — source-file
classification, description derivation, prompt-diff derivation, the attribution
sanity gate, and full-vs-incremental mode selection — into four generated crates,
all converged on attempt 1, all swapped in behind the unchanged I/O shell,
conformance byte-identical throughout. `cmd_gen`'s hand-written surface is now the
irreducible I/O skeleton plus a regression corpus. The generator's decisions are
generated; what sequences them is the shell — which is the honest shape of the
fixpoint.

## THE FIXPOINT — the toolchain regenerates itself (2026-07-24)

This is the end-to-end self-hosting proof: force-regenerate **every** self-host
module from prose with the stage1 binary, rebuild the toolchain from the freshly
regenerated crates (**stage2**), and run the complete gate battery against stage2.
The fixpoint criterion is behavioral (conformance corpus + equivalence + unit
tests); byte-identity of goldens is the conformance gate and is never allowed to
move.

**Result: the fixpoint holds — and holds in its strongest form.** All 36 modules
force-regenerated to a **byte-identical** result (`gen(src) == src`), the stage2
binary is bit-identical to stage1, and every gate is green:

| Gate | Result |
| --- | --- |
| 1. Conformance (stage2 binary, goldens untouched) | **47 / 47 byte-identical** |
| 2. Equivalence harness | **226 / 226 green** |
| 3. `cargo test --workspace` (rust/) | **245 / 245** |
| 4. `cargo clippy --workspace --all-targets` | **clean (0)** |
| 5. `napl status` (selfhost/) | **36 / 36 clean** |
| 6. workspace-root `cargo test` (`selfhost/.napl/src/rust`) | **419 / 419** |

### Run shape

- **Engine:** the coding-agent engine was set to **opus** (`selfhost/.napl/lock.json`
  `model: opus`) for the whole force sweep; the LLM derivation loops (IR / attribution
  / machine-layer) run on the same engine.
- **Procedure:** `napl gen rust --module <m> --force` per module, strictly
  foreground, sequenced in dependency order (leaves first) so every regenerated
  dependency is on disk before its dependents regen against it.
- **Order:** wave-1 leaves (`hash`, `parse_output`, `body_lines`, `extensions`,
  `text_diff`, `drift`, `scanner`, `targets`, `guard`, `schemas_line_range`,
  `schemas_ordered_map`, `schemas_ir`, `schemas_frontmatter`) → wave-2
  (`schemas_attribution`, `schemas_lock`, `schemas_map`, `schemas_ml`, `blame`,
  `reverse`) → wave-3 (`schemas_journal`, `incremental`, `yaml`, `prompts`) → the 13
  cli slices (`clock_fmt`, `paths_core`, `statusclass_render`, `snapshot_diff`,
  `snapshot_filter`, `watch_filter`, `gen_classify`, `gen_mode`,
  `driftdetect_replay`, `blame_render`, `gen_prompt_diff`, `gen_attribution_check`,
  `reconcile_derive`).

### Per-module attempts

**Every one of the 36 modules converged on code-gen attempt 1 of 3.** No module
entered the escape hatch; the escape-hatch list is **empty**. Each force-regen with
an unchanged prompt was a genuine no-op: the coding agent ran, executed the module's
`cargo test` gate at the workspace root, and confirmed the existing generated crate
already satisfies its prompt, so it produced **0 file patches** (verified in the
journal: all 36 opus-run gens recorded empty `files`). Attribution and the machine
layer were re-derived fresh on every gen.

Non-fatal derivation retries (they do **not** count against the 3 code-gen attempts,
and every one resolved automatically):

- **Attribution retry (Cargo.toml-outside-set), succeeded attempt 2:** `hash`,
  `blame`. The first attribution pass mapped a line to `<crate>/Cargo.toml`, which is
  outside the attributed file set; the retry dropped it.
- **Machine-layer YAML retry, succeeded attempt 2:** `scanner`, `targets`,
  `schemas_lock`, `reverse`, `gen_prompt_diff`, `gen_attribution_check`.
- **Best-effort IR skipped (gen continues):** `parse_output`, `prompts` — the IR
  derivation hit a quoted-scalar parse error and was skipped; IR is best-effort and
  does not gate the fixpoint.

### The honest finding — this is a *literal* fixpoint, not a rewrite

The run's most important, and most honest, result: **the opus force-sweep changed no
generated source.** Regeneration is nominally non-deterministic, so new code text
*could* differ — but a strong coding agent, handed an unchanged prompt and an
existing generated crate that already passes its tests, correctly makes no edits.
Every module was therefore already a **fixed point of the generator**: applying the
generation operator reproduces the exact same source, byte for byte, for all 36
modules. That is why the stage2 `cargo build --release` recompiled nothing (finished
in ~1s) and the stage2 binary is bit-identical to stage1.

This is a *stronger* statement than the behavioral fixpoint the campaign set out to
prove (corpus + equivalence). It does mean the sweep did not exercise "regenerate a
module from scratch and reconverge" — the toolchain's `--force` runs the agent
against the *existing* code, it does not delete-then-regenerate — so the demonstrated
property is stability under regeneration (`gen(src) = src`), which is exactly the
mathematical definition of a fixpoint.

### Prompt tightenings (the valuable output) — made in step 0, before the sweep

Two prompts were tightened to close small debts before the sweep; both are behavior
pins that make the prompt the real spec. (These were the only source-changing gens;
the opus sweep re-ran both as no-ops, confirming the tightened prompts are fixpoints.)

1. **`blame_render` — the unowned `Move` match arm.** `mode_str` must exhaustively
   match `JournalMode`, whose `schemas_journal` prompt owns a `Move` variant, so the
   generated `JournalMode::Move => "move"` arm existed but the `blame_render` prompt
   named only `Full`/`Incremental`/`Reconcile` — the arm was generated-but-unspecified.
   The prompt now pins `Move → "move"` (and notes the match is exhaustive), the
   attribution now formally owns the arm (lib lines 10–18 ← the amended `mode_str`
   prose), and the equivalence harness asserts the `Move` label.

2. **`schemas_frontmatter` — the `crate:` key promoted into the strict schema.** The
   optional `crate:` frontmatter key (member-crate grouping) was previously read by a
   hand-rolled second `serde_yaml` pass in `discovery::declared_crate`, invisible to
   the strict `Frontmatter` parse. The prompt now makes `crate` a **known** optional
   field, deserialized (`#[serde(default, rename = "crate")]`) into a public
   `crate_name: Option<String>` (defaulting to `None`); the unknown-field policy is
   unchanged (the schema was already permissive). `discovery::declared_crate` now
   reads it straight off the strict parse (the hand-rolled YAML scan is deleted). The
   napl-core adapter re-exports the field automatically; only two hand-written struct
   literals (napl-core `prompts.rs` test `fm()`, equivalence `prompts.rs` `fm()`) and
   one equivalence harness literal needed `crate_name: None` added.

**Shape-change cascade (a fixpoint finding).** Adding a field to `Frontmatter` breaks
every struct literal that constructs it — including the `prompts` sibling crate's
test helper in the *same* generated workspace. Because the rust target's in-gen gate
is a **whole-workspace `cargo test`**, a leaf shape-change makes the leaf's own gate
fail until its dependents are regenerated. When `schemas_frontmatter` was regenerated
with the new field, the coding agent (working in-workspace) updated the `prompts`
sibling's literal to `crate_name: None` to make the workspace compile, which
pre-staged `prompts` for its own clean regen. The lesson for the fixpoint: a
**shape-changing** regen of a widely-constructed type is not a per-module leaf
operation — it must be landed together with its constructors, and the leaves-first
ordering only works for shape-*preserving* regens.

### Equivalence bridge updates (signature bridges, semantics unchanged)

Allowed per the campaign rule (a regen that changes a bridged name/type may need the
harness bridge updated; semantics must not weaken):

- `selfhost/equivalence/tests/prompts.rs` — `fm()` `Frontmatter` literal gained
  `crate_name: None` (mechanical, from the `crate_name` field addition).
- `selfhost/equivalence/tests/schemas_frontmatter.rs` — **strengthened** (not
  weakened): added a `crate_name == None` default assertion and a new case asserting
  `crate: shared` parses to `crate_name == Some("shared")`.
- `selfhost/equivalence/tests/blame_render.rs` — **strengthened**: `mode_str` now
  asserts the `Move → "move"` label.

Equivalence went 225 → **226** (the one net-new case is the `crate_name` capture
assertion). No bridge relaxed a check.

### Escape-hatch list

**Empty.** All 36 modules force-regenerated and passed every gate; none failed 3
attempts, none was left stale.

### Verdict — self-hosting demonstrated end-to-end

Stage2 — the toolchain rebuilt from crates the stage1 binary regenerated from prose,
under the opus engine — passes the complete 47-scenario conformance corpus
**byte-identically**, the 226-case equivalence harness, the 245-test rust workspace,
clippy, `napl status` 36/36, and the 419-test generated workspace. Because every
force-regen was a byte-identical no-op, the generated source tree is a **literal
fixed point** of the generator, and stage2 is bit-identical to stage1. The toolchain
regenerates itself and still passes every corpus. **Self-hosting is demonstrated
end-to-end; deleting the hand-written implementation (rust-final) is unblocked** —
subject to the still-hand-written surface catalogued below staying hand-written by
design.

### What remains hand-written (the rust-final boundary)

The fixpoint covers every **generated** module (all of `napl-core`'s pure logic and
the extracted pure cores of `napl-cli`). What is *not* generated, and stays
hand-written by design, is the irreducible I/O and glue — this is the precise surface
that "rust-final deletion" must **not** delete:

- **`napl-core` adapters** (`rust/crates/napl-core/src/**`): thin re-export/`map_err`
  adapters over the generated crates, plus the `SchemaError` seam and the
  hand-written unit corpora that ride along as the regression net. ~151 adapter LOC.
- **`napl-cli` I/O shell** (~3,437 LOC across 18 modules): `cmd_gen`'s
  `run_gen_locked` orchestrator + `run_attempts`/`retry_for_change`/`derive_*` LLM
  loops, `process` (subprocess + lockfile), `fsutil`, `error`, `state`, `main`
  (arg-parse/dispatch), and the fs shells of the `cmd_*` handlers. The pure cores of
  these are already generated crates re-exported behind unchanged call sites;
  `discovery.rs` (module-identity + the now-strict `declared_crate`) is hand-written
  shell.
- **`napl-lsp`** (~1,800 LOC): the tower-lsp JSON-RPC server (`backend`,
  `navigation`, `hover`, `diagnostics`, `state`); its pure-enough slices (`ml`,
  `classify`, `convert`, `context`) are a later phase, gated by the LSP integration
  suite, not by this fixpoint.
- **Toolchain-owned generated-tree scaffolding**: the virtual `[workspace]` root
  manifest and the per-crate `Cargo.toml`/`lib.rs` grouping files the toolchain
  writes and owns.

None of these are self-host units under the current gates; deleting `rust-final`
means deleting any *remaining hand-written module bodies whose behavior is now proven
generated*, not this I/O skeleton.

## Deps-enforcement slice — gen-time sibling-dependency gate (2026-07-24)

The fixpoint above proved every module regenerates byte-identically *given a correct
prompt*. This slice closes the gap the fixpoint could not see: a prompt that leaves
the sibling-crate relationship implicit lets a regen agent quietly **reimplement** a
sibling's types instead of depending on them — still compiling, still passing tests,
but no longer composing on the workspace. The slice adds a machine gate and hardens
the prompts against that failure mode.

### What is enforced

After a generated module's `cargo test` passes, gen inspects the `[dependencies]`
table of that module's generated `Cargo.toml`. Every **path dependency on a sibling
member crate** must also be declared in the prompt's `deps:` frontmatter. An
undeclared sibling path-dep fails the gen attempt (it retries with explicit feedback,
then fails the run). Non-sibling (crates.io) deps are not policed; a declared-but-unused
dep is not an error; grouped crates whose `Cargo.toml` the toolchain owns are skipped.

The pinned error (stderr, as `napl: {message}`):

> gen failed for module '{module}' ({target}): the generated Cargo.toml declares a
> path dependency on the sibling crate '{dep}', which is not declared in the prompt's
> `deps:` frontmatter — declare it in `deps:` or remove the dependency.

### The pure/shell split

The decision is a pure function pair — `cargo_path_dep_crates(cargo_toml: &str)`
(parses only `[dependencies]`, returns the sorted path-dep crate names) and
`check_declared_deps(module, target, path_dep_crates, declared)` (the pure verdict,
carrying the pinned message). Only the `Cargo.toml` read lives in the I/O shell
(`deps_gate_error`). The pure core is written to be a future `.napl` module; the shell
stays hand-written, matching the campaign's pure/IO discipline.

### Conformance 47 → 48

New scenario `81-gen-undeclared-dep`: an agent writes `adder/Cargo.toml` with a path
dep on `helper` but leaves `deps:` empty; the run must exit 1, print the per-attempt
`undeclared sibling dependency in Cargo.toml` lines, emit the pinned stderr message,
and leave the tree unlocked with no attribution/map/journal/gen.lock written. The 47
existing goldens are byte-identical; the corpus is now **48 / 48**.

### The prose-trim policy — dep-enumeration is redundant, behavioral imperatives are spec

Fifteen "Builds on…" prompt sections carried verbose dependency prose. The slice
trimmed the **redundant** half and kept the **load-bearing** half, on a sharp rule:

- **WHERE a dependency exists = redundant.** "Add a path dependency on `../foo` in
  your `Cargo.toml`; it is a workspace sibling, not a crates.io dependency, so add it
  even though the general guidance is to avoid outside dependencies." The `deps:`
  frontmatter and the generated task text (`Declared dependencies: …`) already carry
  this. Trimmed.
- **HOW to relate to it = spec, must stay.** "Use its public API — do not reimplement
  its types or logic, and do not depend on any hand-written crate." This is the only
  thing standing between the agent and a local reimplementation. Kept (or restored in
  compact one-sentence form where a prior trim had removed it).

### The `incremental` regression — the campaign's honest finding

The first pass of the trim was too aggressive. On `incremental` it deleted the entire
imperative — *"Add a path dependency on each in your `Cargo.toml`… use their public
API, and do not reimplement their logic or depend on any hand-written crate"* — leaving
the two sibling bullets as pure type **descriptions**. On regen (journal gen #93) the
agent did exactly what an unconstrained prompt invites: it **reimplemented** `LineRange`
and `AttributionEntry` locally in `incremental/src/lib.rs` and emptied its `Cargo.toml`
`[dependencies]` (dropping `schemas_line_range` + `schemas_attribution`). It compiled
and passed tests — a silent loss of composition that only a human diff caught.

The fix: the regressed gen #93 was **reverted** to its pre-#93 accepted state (generated
crate, per-module metadata, and the surgical removal of just the #93 journal line and
the `incremental` entry in `map.json`; the shared journal/map files' good #87–#92
entries were preserved). The audit of all 15 prompts then restored the compact
"use-the-sibling / don't-reimplement" imperative in the **9** prompts whose trim had
removed the only statement of it (`incremental`, `blame`, `prompts`, `yaml`,
`schemas_attribution`, `schemas_journal`, `schemas_lock`, `schemas_map`, `schemas_ml`);
the other **6** (`blame_render`, `gen_attribution_check`, `gen_prompt_diff`,
`reconcile_derive`, `reverse`, `driftdetect_replay`) kept the trim because their
per-dependency bullets still carry an explicit "Use its public X … do not reimplement"
directive. All 14 prompt-stale modules then regenerated as **clean no-ops** (journal
gens #93–#106, 0 file patches each) — `incremental` included, this time depending on its
siblings rather than reimplementing them.

The lesson is the reusable one: **a prompt that only *describes* a sibling's types
invites reimplementation; a prompt must *instruct* the agent to depend on and use them.**
The machine gate catches the Cargo.toml symptom; the prompt imperative prevents the
cause.

### Gate results (post-slice)

| Gate | Result |
| --- | --- |
| 1. Conformance (new `81-gen-undeclared-dep`; 47 goldens untouched) | **48 / 48** |
| 2. Equivalence harness | **226 / 226** |
| 3. `cargo test --workspace` (rust/) | **all green** |
| 4. `cargo clippy --workspace --all-targets` | **clean (0)** |
| 5. `napl status` (selfhost/) | **36 / 36 clean** |
| 6. workspace-root `cargo test` (`selfhost/.napl/src/rust`) | **419 / 419** |
