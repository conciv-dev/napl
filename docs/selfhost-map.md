# Self-Host Module Map — the campaign tracker

This is the dependency-ordered inventory of the toolchain being re-expressed as
`.napl` modules. It is the tracker for the self-host campaign: every module the
stage0 binary must eventually regenerate from prose, ordered so a generated
module is only depended on by a later gen once it has passed its equivalence
gate.

The unit of self-hosting is a **module** (`rust/crates/napl-core/src/<m>.rs` or
`schemas/<m>.rs`). Each module's hand-written `#[cfg(test)] mod tests` suite is
its **equivalence corpus** — the exact input→output cases the generated code must
reproduce (behaviorally, not byte- or type-identically).

## The pure/IO split decides the phase order

`napl-core` is **entirely pure**: no module under `napl-core/src/` touches
`std::fs`, `std::process`, `std::io`, or `std::env`. It is deterministic
data-in/data-out logic, which is exactly what the behavioral-equivalence gate can
prove. This is why the whole self-host campaign starts and lives in `napl-core`.

All **I/O lives one crate up**, in `napl-cli` (`cmd_*`, `fsutil`, `process`,
`paths`, `clock`) and in `napl-lsp` (the tower-lsp server). Those modules drive
filesystems, subprocesses, and a JSON-RPC loop; their "corpus" is the conformance
suite and the LSP integration tests, not per-function unit vectors. They are a
**later phase** and are inventoried here only for completeness.

## Stage1 swap-in status (SHIPPING)

Phase 1 is not just self-hosted in the harness — it is **swapped in**: the shipping
`napl` binary now runs the generated crates. `napl-core`'s hand-written module
bodies are deleted and replaced by thin adapters over the generated crates
(cross-workspace path-deps; `selfhost/` untouched). **22 of 23 modules run
generated code; conformance is 40/40 byte-identical.** Every "done" below is now
**swapped-in**, with one exception on the escape-hatch list:

- **`schemas::journal` — escape-hatch (hand-written body restored).** Generated
  `read_journal_str` emits corrupt-line warning text that diverges from what
  conformance `34-journal-corrupt-line` pins; the equivalence gate only checks
  `(entries, warnings.len())`, so this is a real observable gap outside the adapter
  spec. Generated `schemas_journal` stays equivalence-green (8/8), just not wired
  into the binary. (`schemas::frontmatter` was NOT escape-hatched — its
  `FrontmatterError` is bridged to the pinned message text in the adapter, because
  the generated `prompts` crate composes on `schemas_frontmatter::Frontmatter`.)

See `selfhost.md` → "Stage1 swap-in — DONE" for the workspace-membership call, the
adapter seam catalog, and the full gate numbers.

## Phase 1 — `napl-core` (pure; the active campaign)

Waves are dependency layers. Wave 1 = pure leaves with **no intra-crate
dependency**. Wave *n* depends only on waves `< n`. Every `napl-core` module is
pure, so the "pure/IO" axis is uniform here; the differentiator is dependency
depth and corpus size.

### Wave 1 — pure leaves (no intra-crate deps)

| Module | LOC | Unit tests | External crates | Self-host status |
| --- | ---: | ---: | --- | --- |
| `body_lines` | 111 | 5 | — | **done** (pilot, re-genned as workspace member in slice 2, 5/5) |
| `extensions` | 141 | 7 | — | **done** (slice 1) |
| `hash` | 44 | 4 | `sha2`, `hex` | **done** (slice 1) |
| `parse_output` | 39 | 3 | — | **done** (slice 1) |
| `text_diff` | 392 | 11 | — | **done** (slice 1) |
| `drift` | 184 | 3 | — | **done** (slice 2, 3/3) |
| `scanner` | 634 | 12 | — | **done** (slice 2, 12/12) |
| `targets` | 272 | 6 | — | **done** (slice 2, 9/9 — corpus grew with the workspace fields) |
| `guard` | 189 | 5 | `serde` | **done** (slice 2, 5/5) |
| `schemas::frontmatter` | 180 | 6 | — | **done** (slice 2, 6/6) |
| `schemas::ir` | 123 | 6 | — | **done** (slice 2, 6/6) |
| `schemas::line_range` | 159 | 8 | — | **done** (slice 2, 8/8) |
| `schemas::ordered_map` | 163 | 4 | — | **done** (slice 2, 4/4) |

Wave 1 is fully self-hosted: **13/13 modules**, 83 equivalence cases green.

Waves 1–3 together: **23/23 modules, 189 equivalence cases green**, escape-hatch
list still empty — all of `napl-core` self-hosts (phase 1 complete).

### Wave 2 — depends only on wave 1

Wave 2 is fully self-hosted: **6/6 modules, 72 equivalence cases green** (slice 3),
each generated on attempt 1 of 3. Every generated wave-2 crate **path-deps the
generated wave-1 crate(s)** it builds on — it composes on the generated code, not
on hand-written napl-core.

| Module | LOC | Unit tests | Intra-crate deps | Self-host status |
| --- | ---: | ---: | --- | --- |
| `blame` | 303 | 13 | `text_diff` | **done** (slice 3, 13/13 — path-deps generated `text_diff`) |
| `reverse` | 297 | 12 | `body_lines`, `schemas` | **done** (slice 3, 12/12 — path-deps generated `body_lines` + `schemas_attribution` + `schemas_line_range`) |
| `schemas::attribution` | 173 | 9 | `line_range` | **done** (slice 3, 9/9 — path-deps generated `schemas_line_range`) |
| `schemas::lock` | 290 | 19 | `extensions` | **done** (slice 3, 20/20 — path-deps generated `extensions`; +1 empty-model case) |
| `schemas::map` | 553 | 10 | `ordered_map` | **done** (slice 3, 10/10 — path-deps generated `schemas_ordered_map`) |
| `schemas::ml` | 185 | 8 | `line_range` | **done** (slice 3, 8/8 — path-deps generated `schemas_line_range`) |

`blame` was confirmed to depend **only** on `text_diff`: its `BlameSourceEntry`
is a blame-local struct, not a `schemas::journal` type, so no wave-3 journal
pull-forward was needed. (`schemas::journal` depends on `blame`, not the reverse.)

### Wave 3 — aggregates over waves 1–2

Wave 3 is fully self-hosted: **4/4 modules, 34 equivalence cases green** (slice 4),
each generated on **attempt 1 of 3**. Every generated wave-3 crate path-deps the
generated wave-1/2 crate(s) it builds on.

| Module | LOC | Unit tests | Builds on (generated crate) | Self-host status |
| --- | ---: | ---: | --- | --- |
| `schemas::journal` | 228 | 8 | `blame`, `text_diff` | **done** (slice 4, 8/8) — **escape-hatch at stage1 swap-in** (warning-text divergence; hand-written body ships) |
| `incremental` | 235 | 2 | `schemas_attribution`, `schemas_line_range` | **done** (slice 4, 3/3 — 2 corpus + 1 composition case) |
| `yaml` | 535 | 9 | `schemas_attribution`, `schemas_ir`, `schemas_ml`, `schemas_line_range` | **done** (slice 4, 9/9 — byte-exact block goldens) |
| `prompts` | 523 | 7 | `schemas_attribution`, `schemas_frontmatter`, `schemas_line_range`, `targets` | **done** (slice 4, 14/14 — 7 corpus + 7 byte-exact pins) |

`lib.rs` (23 LOC, 0 tests) is a pure re-export root and is not a self-host unit.

**Phase 1 (`napl-core`) is COMPLETE: 23/23 modules, 189 equivalence cases green,
every module converged on attempt 1** — and now **SWAPPED IN**: the shipping binary
runs generated code for 22 of 23 modules, conformance 40/40 byte-identical, with
`schemas::journal` on the stage1 escape-hatch (warning-text divergence). See
`selfhost.md` → "Stage1 swap-in — DONE".

## Phase 2 — `napl-cli` (I/O orchestration; later)

Not behavioral-unit self-hostable in the same way — these are gated by the
conformance corpus (`conformance/`, 40 scenarios), not per-function vectors. A few
carry real unit tests and could be pulled forward as pure leaves.

| Module | LOC | Unit tests | Character |
| --- | ---: | ---: | --- |
| `statusclass` | 213 | 2 | classification — **stays phase 2** (see note); its 2 tests are pure-render only |
| `driftdetect` | 146 | 2 | mostly pure over journal data |
| `snapshot` | 147 | 2 | fs walk + hashing (I/O) |
| `fsutil` | 70 | 2 | fs read/write (I/O) |
| `paths` | 126 | 2 | path algebra (mostly pure) |
| `process` | 435 | 4 | subprocess spawning (I/O) |
| `clock` | 65 | 3 | time (I/O) |
| `state` | 89 | 0 | in-memory state |
| `error` | 35 | 0 | error type |
| `cmd_*` (gen/status/init/watch/reconcile/blame/build/test) | ~1900 | 0 | command handlers (I/O + orchestration) |
| `main` | 184 | 0 | arg parsing / dispatch |

`cmd_gen` (1084 LOC) is the stage0 orchestrator itself — the last thing to
self-host, and the true fixpoint when it does.

**Slice-4 call on `statusclass` and `napl-lsp`'s `classify`.** Both were candidates
to pull forward into phase 1. Reading the source settled it: both drag I/O — their
`detect_drift` reads generated files off disk (`fsutil::exists` /
`std::fs::read_to_string` / `Path::exists`), so the module as written is not pure
and cannot be gated by the behavioral-equivalence harness. `statusclass`'s **two
unit tests are pure** (`StatusEntry::line` padding and `is_error`), and `classify`
carries **no unit tests at all**. So neither folds into phase 1 as written. The
pure rendering slice of `statusclass` (the `line()`/`is_error()` corpus) could be
pulled forward later only if the module is split so the fs-reading classifier lives
elsewhere; until then both stay in their I/O phases (2 and 3).

## Phase 3 — `napl-lsp` (JSON-RPC server; later)

Gated by the LSP `integration` suite (12 tests). `classify`, `ml`, `convert`,
`context` are pure enough to be pulled forward; `backend`/`navigation`/`hover`/
`diagnostics` are server plumbing.

| Module | LOC | Unit/integration tests | Character |
| --- | ---: | ---: | --- |
| `integration` | 154 | 12 | end-to-end LSP corpus |
| `ml` | 144 | 4 | pure machine-layer parsing |
| `hover` / `navigation` / `diagnostics` / `backend` / `context` / `classify` / `convert` / `state` / `testkit` / `lib` | ~1500 | 0 | server plumbing |

## Fixpoint definition

The toolchain self-hosts when, for every non-escape-hatch module, stage0-generated
code passes that module's hand-written unit corpus under the shared equivalence
harness. Byte-identity is never required. The campaign is complete when the
generated toolchain can regenerate itself and still pass every corpus.

## Escape-hatch list

Modules that stay hand-written because current stage0 + prompt cannot reproduce
their behavior under the equivalence gate. A module leaves the list only when its
prompt drives a passing generation.

- **Generation:** *(empty)* — no `napl-core` module has failed to converge (23/23
  on attempt 1; phase 1 complete).
- **Stage1 swap-in:** `schemas::journal` — the generated `read_journal_str`'s
  corrupt-line **warning text** diverges from the format conformance
  `34-journal-corrupt-line` pins byte-for-byte (the equivalence gate only compares
  `(entries, warnings.len())`, never the text). Hand-written body restored in the
  shipping binary; generated `schemas_journal` remains equivalence-green and
  drift-clean. This is a message-format gap, not a logic gap, and it cascades
  nowhere (no generated crate depends on `schemas_journal`).

## Layout note (RESOLVED in slice 2 — Cargo workspace)

The stage0 gen target dir is fixed at `.napl/src/rust/` per target, and generated
files are locked `0444`. The pilot put `body_lines` **at the crate root**, and
slice 1 landed later modules as **nested package crates** in subdirectories — a
layout that composed only because Cargo silently ignores a nested package from the
root, which also meant the in-gen `cargo test` gate covered only the root crate.

Slice 2 replaced this with a real **Cargo workspace**. The rust target adapter now
carries `workspace_layout = true` and an `attribution_exclude_root_files` list;
the toolchain writes and owns the workspace root `Cargo.toml` (a virtual
`[workspace]` manifest listing every member crate), refreshing it on each gen so a
new module is added as a member before its `cargo test` runs. The root manifest is
treated like the guard files (AGENTS.md/CLAUDE.md): toolchain-owned, excluded from
attribution (root-level only — per-module `Cargo.toml` files stay attributed), not
locked, not drift-checked. Every module — `body_lines` included — is now a uniform
member crate at `.napl/src/rust/<module>/`, and the in-gen `cargo test` runs at the
workspace root, covering **all 13 members** in one gate. The shared equivalence
harness (`selfhost/equivalence/`) remains the behavioral cross-module gate.
</content>
</invoke>
