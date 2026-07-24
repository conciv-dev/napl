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

## Verdict

The loop is **viable for scaling**. A behavior-only prompt with its test corpus
as data drove stage0 to generate a Rust module that is behaviorally equivalent to
the hand-written original on its full unit-test corpus, on the first attempt,
fully attributed, locked, and drift-clean. The remaining work is breadth
(dependency-ordered modules, a shared corpus harness) and tightening prompts where
prose under-specifies a contract — not a missing capability in the loop itself.
