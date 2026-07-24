# NAPL doc site — design spec

Date: 2026-07-24. Status: approved by Omri.

## Goal

Public documentation + showcase site for NAPL. Structural clone of the conciv
widget site (`aidx/apps/site`): fumadocs on TanStack Start, deployed as a
Cloudflare Worker. Centerpiece: an interactive playground that runs real NAPL
machinery in the browser and a self-host showcase built from the repo's own
`selfhost/` data.

## Decisions (locked)

| Decision | Choice |
| --- | --- |
| Location | `apps/site` in this monorepo (next to `apps/vscode`) |
| Framework | TanStack Start + fumadocs-mdx/fumadocs-ui 16 + Tailwind 4 + shiki 4 — mirror `~/Public/web/aidx/apps/site` |
| Deploy | Cloudflare Worker via wrangler, `napl-site` + `napl-site-preview` (conciv-site pattern) |
| Playground runtime v1 | WASM napl-core (live) + replayed real gen sessions (fixtures) |
| Playground runtime v1.5 | In-browser local LLM via `@huggingface/transformers` (WebGPU), user-selectable model |
| Editor | CodeMirror 6 direct (no Sandpack bundler; Sandpack-style look) |
| Content v1 | Landing w/ hero playground, language guide, CLI reference, self-host showcase |

## Components

### 1. Site scaffold (`apps/site`)

Clone the aidx site's shape: `source.config.ts` (fumadocs-mdx, mermaid remark
plugin, processed-markdown postprocess), `src/routes` (docs tree, `llms-full.txt`
route, landing index), `src/components` (mdx map, landing animation components as
needed), Tailwind 4, vitest + e2e configs, `wrangler.jsonc` with prod + preview
environments. Joins the pnpm workspace and turbo pipeline.

### 2. Grammar (`packages/napl-grammar`)

- `napl.tmLanguage.json` — TextMate grammar: YAML frontmatter island (embed
  YAML), markdown body (embed markdown), highlighted `given`/`expect` test
  blocks, module/deps/targets keys.
- `mapl.tmLanguage.json` — machine-margin files; entry kinds colored by
  severity: `ambiguity` (error), `assumption`/`no-op` (warning), `note` (info).
- Shiki registration helper (`loadNaplLanguages(highlighter)`) used by the
  site's MDX code blocks. Emoji aliases `.🧑`/`.🤖` map to the same grammars.
- The same TM files later ship in `apps/vscode` — grammar spec prose in the
  package README is the source of truth.
- CodeMirror cannot run TM grammars: the live editor uses a small CM6
  StreamLanguage implementation of the same spec (lives with the playground,
  §4). Two implementations, one spec — accepted tradeoff (Monaco rejected as
  too heavy).

### 3. WASM runtime (`rust/crates/napl-wasm` → `packages/napl-wasm`)

wasm-bindgen bindings over napl-core (which is itself generated code — say so
on the site): scan/frontmatter parse + validation diagnostics, body-line
mapping, attribution lookup, blame replay over a supplied journal, drift
detection against supplied hashes, mapl parsing. Built with wasm-pack; consumed
by the site as a workspace package. No I/O — callers pass file contents.

### 4. Playground (`<NaplPlayground>` MDX component)

CodeMirror 6, file tabs (prompt `.napl` / generated src / `.mapl`), live WASM
diagnostics (frontmatter errors as squiggles, mapl entries as margin marks),
hover: prompt sentence ↔ generated lines via attribution data.

**GenEngine contract (day one):**

```ts
interface GenEngine {
  run(task: string, files: Record<string, string>): AsyncIterable<GenEvent>
}
type GenEvent = task | file-edit | diff | attribution | mapl-entry | lock | error
```

- v1 `ReplayEngine`: streams pre-recorded real sessions. Fixtures are built at
  build time by a script reading real `selfhost/` + `examples/` state
  (journal, attribution, mapl) — never hand-authored.
- v1.5 `TransformersEngine`: `@huggingface/transformers`, WebGPU, model picker
  (SmolLM/Qwen/Phi class), pattern copied from aidx
  `src/components/landing/demo/local-model.ts` (browser-cached weights,
  progress events). The model performs the real agent contract ("task + files
  in, edited files out"); WASM napl-core does snapshot diff, hashing, locking,
  drift; derivations reuse the real system-prompt constants from the generated
  `prompts` crate. UI is engine-agnostic; copy frames local-LLM gen as
  toy-scale ("real gens use your CLI agent").

### 5. Self-host showcase (`/selfhost` route)

Module browser over the real self-hosted modules: prompt left, generated Rust
right, attribution hover-linking both directions, gen blame timeline, mapl
margin notes. Data = build-time JSON from the repo tree. No server.

### 6. Landing

conciv-site-style animated hero + embedded hero playground (curated greeting
example, replayed gen).

### 7. Content v1

- Language guide: prompt format, frontmatter tests, gen loop, drift/reconcile
  discipline, machine layer, emoji aliases.
- Toolchain reference: init/gen/status/blame/test/reconcile/watch/lsp, install
  page (npm `napl-lang`, curl, cargo).
- Self-host story page (prose companion to the showcase).

### 8. Testing

Mirror conciv-site: vitest (fixture builder, grammar snapshot tests, wasm
smoke via node), e2e config for playground interaction.

## Phasing

1. **Scaffold + grammar + content skeleton** — site builds & deploys with
   static highlighting. (Parallel: opus scaffolds site; codex builds grammar
   package.)
2. WASM crate + CM6 editor + StreamLanguage highlighting.
3. ReplayEngine + fixture builder + self-host showcase.
4. Landing polish + deploy.
5. TransformersEngine + model picker (near-future).

## Non-goals (v1)

Server-side gen, auth, search beyond fumadocs defaults, versioned docs,
mobile-optimized editor (read-only fallback acceptable).
