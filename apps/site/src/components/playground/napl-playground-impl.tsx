import {
  createReplayEngine,
  PlaygroundShell,
  type EditorDiagnostic,
  type HoverContext,
  type PlaygroundFile,
  type ReplayEngine,
  type ReplaySnapshot,
} from '@napl/editor'
import '@napl/editor/styles.css'
import {useCallback, useEffect, useMemo, useRef, useState, type ReactElement} from 'react'
import {
  attributionAtFileLine,
  attributionAtPromptLine,
  ensureNaplWasm,
  parseFrontmatterDiagnostics,
} from '@/lib/napl-wasm.browser'
import type {ShowcaseModule} from '@/lib/fixtures'
import {ReplayMargin} from './replay-margin'

export interface NaplPlaygroundClientProps {
  module: ShowcaseModule
  compact?: boolean
  speed?: number
}

export function NaplPlaygroundClient({
  module,
  compact = false,
  speed = 1,
}: NaplPlaygroundClientProps): ReactElement {
  const [wasmReady, setWasmReady] = useState(false)
  const [prompt, setPrompt] = useState(module.prompt.content)
  const [snapshot, setSnapshot] = useState<ReplaySnapshot | null>(null)
  const [activeFile, setActiveFile] = useState(module.prompt.file)
  const activeFileRef = useRef(activeFile)
  activeFileRef.current = activeFile

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
    (context: HoverContext): string | null => {
      if (!wasmReady || !module.attributionYaml) return null
      const line = context.line + 1
      if (activeFileRef.current === module.prompt.file) {
        const spans = attributionAtPromptLine(module.attributionYaml, line)
        if (spans.length === 0) return null
        return spans
          .map((span) => `→ ${span.file}:${span.lines.start}–${span.lines.end}\n${span.note}`)
          .join('\n\n')
      }
      const spans = attributionAtFileLine(module.attributionYaml, activeFileRef.current, line)
      if (spans.length === 0) return null
      return spans
        .map(
          (span) =>
            `← prompt ${span.prompt_lines.start}–${span.prompt_lines.end}\n${span.note}`,
        )
        .join('\n\n')
    },
    [wasmReady, module.attributionYaml, module.prompt.file],
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

  const files = useMemo<PlaygroundFile[]>(() => {
    const generated = module.files.map((file) => ({
      name: file.path,
      language: 'source' as const,
      content: editedPaths.has(file.path) ? state?.files[file.path] ?? '' : file.content,
      readOnly: true,
    }))
    return [
      {name: module.prompt.file, language: 'napl' as const, content: prompt},
      ...generated,
    ]
  }, [module.files, module.prompt.file, prompt, state, editedPaths])

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
      <div className="napl-playground__toolbar">
        <button
          type="button"
          className="napl-playground__gen"
          onClick={onGen}
          disabled={!wasmReady}
        >
          {running ? 'Pause' : started && !snapshot?.done ? 'Resume gen' : 'Run gen'}
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
            ? `${snapshot?.position ?? 0}/${engine.length} · ${module.target}`
            : 'loading wasm…'}
        </span>
      </div>
      <PlaygroundShell
        title={`${module.module}.napl`}
        files={files}
        activeFile={activeFile}
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
