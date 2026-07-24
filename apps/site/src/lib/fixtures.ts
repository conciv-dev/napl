import type {RecordedSession} from '@napl/editor'
import indexData from '../fixtures/showcase-index.json'

export type FixtureCollection = 'selfhost' | 'example'

export interface ShowcaseAttribution {
  promptLines: [number, number]
  file: string
  lines: [number, number]
  note: string
}

export interface ShowcaseMapl {
  promptLines: [number, number]
  kind: 'ambiguity' | 'assumption' | 'note' | 'no-op'
  severity: 'error' | 'warning' | 'info'
  message: string
  reasoning?: string
  suggestion?: string
}

export interface ShowcaseFile {
  path: string
  journalPath: string
  content: string
  language: string
}

export interface ShowcaseJournalFile {
  path: string
  journalPath: string
  patch: string
}

export interface ShowcaseJournalEntry {
  gen: number
  timestamp: string
  mode: string
  promptHash: string
  files: ShowcaseJournalFile[]
}

export interface ShowcaseLock {
  path: string
  hash: string
}

export interface ShowcaseModule {
  module: string
  collection: FixtureCollection
  target: string
  targets: string[]
  prompt: {file: string; path: string; content: string}
  promptAtGen: string | null
  summary: string
  files: ShowcaseFile[]
  attribution: ShowcaseAttribution[]
  attributionYaml: string
  mapl: ShowcaseMapl[]
  journal: ShowcaseJournalEntry[]
  lock: ShowcaseLock[]
  session: RecordedSession
}

export interface ShowcaseIndexEntry {
  module: string
  target: string
  promptFile: string
  fileCount: number
  attributionCount: number
  maplCount: number
  genCount: number
  summary: string
}

export const showcaseIndex = indexData as ShowcaseIndexEntry[]

const moduleFiles = import.meta.glob<{default: ShowcaseModule}>(
  '../fixtures/modules/*.json',
)

export async function loadShowcaseModule(name: string): Promise<ShowcaseModule> {
  const loader = moduleFiles[`../fixtures/modules/${name}.json`]
  if (!loader) {
    throw new Error(`unknown fixture module: ${name}`)
  }
  const loaded = await loader()
  return loaded.default
}
