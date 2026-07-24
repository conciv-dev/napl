import {readFileSync, readdirSync} from 'node:fs'
import {resolve} from 'node:path'
import {describe, expect, it} from 'vitest'
import {createReplayEngine, projectState} from '@napl/editor'
import type {RecordedSession} from '@napl/editor'

const fixturesDir = resolve(__dirname, '../src/fixtures')

interface FixtureModule {
  module: string
  collection: string
  target: string
  files: {path: string; content: string}[]
  attribution: unknown[]
  mapl: {kind: string; severity: string}[]
  session: RecordedSession
}

const readJson = <T>(path: string): T =>
  JSON.parse(readFileSync(path, 'utf8')) as T

describe('generated showcase fixtures', () => {
  const index = readJson<{module: string; genCount: number}[]>(
    resolve(fixturesDir, 'showcase-index.json'),
  )

  it('indexes exactly the 36 self-hosted modules', () => {
    expect(index).toHaveLength(36)
    expect(new Set(index.map((entry) => entry.module)).size).toBe(36)
  })

  it('parses every module fixture into a well-formed RecordedSession', () => {
    const files = readdirSync(resolve(fixturesDir, 'modules')).filter((name) =>
      name.endsWith('.json'),
    )
    expect(files.length).toBeGreaterThanOrEqual(36)
    for (const file of files) {
      const module = readJson<FixtureModule>(resolve(fixturesDir, 'modules', file))
      expect(module.module).toBeTruthy()
      expect(module.target).toBeTruthy()
      expect(Array.isArray(module.session.events)).toBe(true)
      expect(module.session.events[0]?.type).toBe('task')
      for (const entry of module.mapl) {
        expect(['error', 'warning', 'info']).toContain(entry.severity)
      }
    }
  })

  it('replays the gen_prompt_diff session to its locked final state', async () => {
    const module = readJson<FixtureModule>(
      resolve(fixturesDir, 'modules', 'gen_prompt_diff.json'),
    )
    const engine = createReplayEngine(module.session, {instant: true})
    const seen: string[] = []
    for await (const event of engine.run()) {
      seen.push(event.type)
    }
    expect(seen).toContain('diff')
    expect(seen).toContain('file-edit')
    expect(seen.at(-1)).toBe('lock')

    const final = projectState(module.session, module.session.events.length)
    expect(final.locked).toBe(true)
    const lib = module.files.find((file) => file.path.endsWith('lib.rs'))
    expect(lib).toBeDefined()
    expect(final.files[lib!.path]).toBe(lib!.content)
  })

  it('exposes the greeting fixture used by the landing hero', () => {
    const greeting = readJson<FixtureModule>(
      resolve(fixturesDir, 'modules', 'greeting.json'),
    )
    expect(greeting.collection).toBe('example')
    expect(greeting.target).toBe('typescript')
    expect(greeting.files.some((file) => file.path.endsWith('greeting.ts'))).toBe(true)
  })
})

describe('attribution hover mapping (the wasm path the playground uses)', () => {
  it('maps a generated line back to its owning prompt sentence, both directions', async () => {
    const wasm = await import('@napl/wasm')
    await wasm.initNaplWasm()
    const greeting = readJson<{attributionYaml: string; prompt: {content: string}}>(
      resolve(fixturesDir, 'modules', 'greeting.json'),
    )

    const reverse = wasm.attributionAtFileLine(greeting.attributionYaml, 'src/greeting.ts', 6)
    expect(reverse.length).toBeGreaterThan(0)
    const promptBodyLine = reverse[0]!.prompt_lines.start

    const forward = wasm.attributionAtPromptLine(greeting.attributionYaml, promptBodyLine)
    expect(forward.some((span) => span.file === 'src/greeting.ts')).toBe(true)

    const docLine = wasm.bodyLineToDocLine(greeting.prompt.content, promptBodyLine)
    expect(docLine).not.toBeNull()
    const roundTrip = wasm.docLineToBodyLine(greeting.prompt.content, docLine as number)
    expect(roundTrip).toBe(promptBodyLine)
  })
})
