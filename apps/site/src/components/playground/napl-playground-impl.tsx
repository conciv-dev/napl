import {
  createReplayEngine,
  PlaygroundShell,
  type EditorDiagnostic,
  type HighlightRange,
  type HoverCard,
  type HoverContext,
  type PlaygroundFile,
  type ReplayEngine,
  type ReplaySnapshot,
} from '@napl/editor'
import '@napl/editor/styles.css'
import {useCallback, useEffect, useMemo, useRef, useState, type ReactElement} from 'react'
import {useTheme} from 'next-themes'
import {ensureNaplWasm, parseFrontmatterDiagnostics} from '@/lib/napl-wasm.browser'
import {
  excerptFromContent,
  promptHighlightsFor,
  resolveGeneratedHover,
  resolvePromptHover,
} from '@/lib/attribution'
import type {ShowcaseModule} from '@/lib/fixtures'
import {ReplayMargin} from './replay-margin'

export interface NaplPlaygroundClientProps {
  module: ShowcaseModule
  compact?: boolean
  speed?: number
  showGen?: boolean
  readOnly?: boolean
}

export function NaplPlaygroundClient({
  module,
  compact = false,
  speed = 1,
  showGen = true,
  readOnly = false,
}: NaplPlaygroundClientProps): ReactElement {
  const [wasmReady, setWasmReady] = useState(false)
  const [prompt, setPrompt] = useState(module.prompt.content)
  const [snapshot, setSnapshot] = useState<ReplaySnapshot | null>(null)
  const [activeFile, setActiveFile] = useState(module.prompt.file)
  const [promptHighlight, setPromptHighlight] = useState<HighlightRange[]>([])
  const [genHighlight, setGenHighlight] = useState<Record<string, HighlightRange[]>>({})
  const activeFileRef = useRef(activeFile)
  activeFileRef.current = activeFile
  const promptRef = useRef(prompt)
  promptRef.current = prompt

  const {resolvedTheme} = useTheme()
  const theme = resolvedTheme === 'light' ? 'light' : 'dark'

  const engineRef = useRef<ReplayEngine | null>(null)
  if (!engineRef.current) {
    engineRef.current = createReplayEngine(module.session, {speed})
  }
  const engine = engineRef.current

  useEffect(() => {
    let active = true
    ensureNaplWasm().then(() => {
      if (active) setWasmReady(true)
    })
    return () => {
      active = false
    }
  }, [])

  useEffect(() => engine.subscribe(setSnapshot), [engine])

  const state = snapshot?.state ?? null
  const running = snapshot?.playing ?? false
  const started = (snapshot?.position ?? 0) > 0

  const diagnostics = useCallback(
    (content: string): EditorDiagnostic[] => {
      if (!wasmReady) return []
      return parseFrontmatterDiagnostics(content).map((diagnostic) => ({
        line: diagnostic.line,
        severity: diagnostic.severity,
        message: diagnostic.message,
      }))
    },
    [wasmReady],
  )

  const hover = useCallback(
    (context: HoverContext): HoverCard | null => {
      if (!wasmReady || !module.attributionYaml) return null
      if (activeFileRef.current === module.prompt.file) {
        const spans = resolvePromptHover(
          context.content,
          module.attributionYaml,
          module.promptAtGen,
          context.line,
        )
        const chosen = spans[0]
        if (!chosen) return null
        setPromptHighlight(promptHighlightsFor(spans))
        const grouped: Record<string, HighlightRange[]> = {}
        for (const span of spans) {
          ;(grouped[span.file] ??= []).push(span.lines)
        }
        setGenHighlight(grouped)
        const excerptFile = module.files.find((file) => file.path === chosen.file)
        return {
          kind: 'card',
          heading: 'Produces',
          excerpt: excerptFile
            ? {code: excerptFromContent(excerptFile.content, chosen.lines), caption: chosen.note}
            : undefined,
          meta: `${chosen.file} · lines ${chosen.lines[0]}–${chosen.lines[1]}`,
          jump: {label: 'Open generated tab ↵', onJump: () => setActiveFile(chosen.file)},
        }
      }
      const active = activeFileRef.current
      const spans = resolveGeneratedHover(
        promptRef.current,
        module.attributionYaml,
        module.promptAtGen,
        active,
        context.line,
      )
      const chosen = spans[0]
      if (!chosen) return null
      setPromptHighlight([chosen.promptDocLines])
      setGenHighlight({[active]: [chosen.lines]})
      return {
        kind: 'card',
        heading: 'Comes from the prompt',
        quote: chosen.sentence || undefined,
        meta: `Prompt · lines ${chosen.promptLines[0]}–${chosen.promptLines[1]} · ${chosen.note}`,
        jump: {label: 'Open prompt tab ↵', onJump: () => setActiveFile(module.prompt.file)},
      }
    },
    [wasmReady, module.attributionYaml, module.prompt.file, module.promptAtGen, module.files],
  )

  const editedPaths = useMemo(
    () =>
      new Set(
        module.session.events
          .filter((event) => event.type === 'file-edit')
          .map((event) => event.path),
      ),
    [module.session],
  )

  const replaying = snapshot
    ? snapshot.playing || (snapshot.position > 0 && !snapshot.done)
    : false

  const files = useMemo<PlaygroundFile[]>(() => {
    const generated = module.files.map((file) => ({
      name: file.path,
      language: 'source' as const,
      content:
        editedPaths.has(file.path) && replaying ? state?.files[file.path] ?? '' : file.content,
      readOnly: true,
    }))
    return [
      {name: module.prompt.file, language: 'napl' as const, content: prompt, readOnly},
      ...generated,
    ]
  }, [module.files, module.prompt.file, prompt, state, editedPaths, replaying, readOnly])

  const activeHighlight =
    activeFile === module.prompt.file ? promptHighlight : genHighlight[activeFile] ?? []

  const onGen = useCallback(() => {
    if (running) {
      engine.pause()
      return
    }
    const firstGenerated = module.files[0]?.path
    if (firstGenerated) setActiveFile(firstGenerated)
    void engine.play()
  }, [engine, running, module.files])

  const onScrub = useCallback(
    (position: number) => {
      engine.pause()
      engine.seek(position)
    },
    [engine],
  )

  const onReset = useCallback(() => {
    engine.pause()
    engine.reset()
    setActiveFile(module.prompt.file)
  }, [engine, module.prompt.file])

  return (
    <div className="napl-playground" data-testid="napl-playground-root" data-playground-root={module.module}>
      {showGen ? (
        <div className="napl-playground__toolbar">
          <button
            type="button"
            className="napl-playground__gen"
            onClick={onGen}
            disabled={!wasmReady || engine.length === 0}
          >
            {running ? 'Pause' : started && !snapshot?.done ? 'Resume' : 'Generate'}
          </button>
          <button type="button" className="napl-playground__ghost" onClick={onReset}>
            Reset
          </button>
          <input
            className="napl-playground__scrub"
            type="range"
            min={0}
            max={engine.length}
            value={snapshot?.position ?? 0}
            onChange={(event) => onScrub(Number(event.target.value))}
            aria-label="Replay position"
          />
          <span className="napl-playground__status">
            {wasmReady
              ? `${snapshot?.position ?? 0}/${engine.length}${module.target ? ` · ${module.target}` : ''}`
              : 'loading wasm…'}
          </span>
        </div>
      ) : null}
      <PlaygroundShell
        title={`${module.module}.napl`}
        files={files}
        activeFile={activeFile}
        theme={theme}
        highlightRanges={activeHighlight}
        onActiveFileChange={setActiveFile}
        onFileChange={(name, content) => {
          if (name === module.prompt.file) setPrompt(content)
        }}
        diagnostics={diagnostics}
        hover={hover}
        output={<ReplayMargin state={state} compact={compact} />}
      />
    </div>
  )
}
