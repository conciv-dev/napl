import {
  createReplayEngine,
  NaplEditor,
  type HighlightRange,
  type HoverContext,
  type ReplayEngine,
  type ReplaySnapshot,
} from '@napl/editor'
import '@napl/editor/styles.css'
import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  type ReactElement,
} from 'react'
import {
  attributionAtFileLine,
  attributionAtPromptLine,
  ensureNaplWasm,
} from '@/lib/napl-wasm.browser'
import {loadShowcaseModule} from '@/lib/fixtures'
import type {ShowcaseModule} from '@/lib/fixtures'

export interface ShowcaseDetailProps {
  name: string
}

export function ShowcaseDetailClient({name}: ShowcaseDetailProps): ReactElement {
  const [wasmReady, setWasmReady] = useState(false)
  const [data, setData] = useState<ShowcaseModule | null>(null)
  const [fileIndex, setFileIndex] = useState(0)
  const [promptHighlight, setPromptHighlight] = useState<HighlightRange[]>([])
  const [genHighlight, setGenHighlight] = useState<HighlightRange[]>([])
  const [snapshot, setSnapshot] = useState<ReplaySnapshot | null>(null)
  const engineRef = useRef<ReplayEngine | null>(null)

  useEffect(() => {
    let active = true
    ensureNaplWasm().then(() => {
      if (active) setWasmReady(true)
    })
    return () => {
      active = false
    }
  }, [])

  useEffect(() => {
    let active = true
    setData(null)
    setSnapshot(null)
    setPromptHighlight([])
    setGenHighlight([])
    setFileIndex(0)
    loadShowcaseModule(name).then((loaded) => {
      if (!active) return
      setData(loaded)
      const preferred = loaded.files.findIndex((file) => file.path.endsWith('.rs'))
      setFileIndex(preferred >= 0 ? preferred : Math.max(0, loaded.files.length - 1))
      engineRef.current = createReplayEngine(loaded.session, {speed: 1.3})
    })
    return () => {
      active = false
    }
  }, [name])

  useEffect(() => {
    const engine = engineRef.current
    if (!engine) return undefined
    return engine.subscribe(setSnapshot)
  }, [data])

  const activeFile = data?.files[fileIndex] ?? null

  const generatedContent = useMemo(() => {
    if (!activeFile) return ''
    const edited = data?.session.events.some(
      (event) => event.type === 'file-edit' && event.path === activeFile.path,
    )
    const replayed = snapshot?.state.files[activeFile.path]
    if (edited && snapshot && (snapshot.playing || (snapshot.position > 0 && !snapshot.done))) {
      return replayed ?? ''
    }
    return activeFile.content
  }, [activeFile, snapshot, data])

  const promptHover = useCallback(
    (context: HoverContext): string | null => {
      if (!wasmReady || !data?.attributionYaml || !activeFile) return null
      const spans = attributionAtPromptLine(data.attributionYaml, context.line + 1)
      const forFile = spans.filter((span) => span.file === activeFile.path)
      setGenHighlight(forFile.map((span) => [span.lines.start, span.lines.end]))
      if (spans.length === 0) return null
      return spans
        .map((span) => `→ ${span.file}:${span.lines.start}–${span.lines.end}\n${span.note}`)
        .join('\n\n')
    },
    [wasmReady, data, activeFile],
  )

  const generatedHover = useCallback(
    (context: HoverContext): string | null => {
      if (!wasmReady || !data?.attributionYaml || !activeFile) return null
      const spans = attributionAtFileLine(data.attributionYaml, activeFile.path, context.line + 1)
      setPromptHighlight(spans.map((span) => [span.prompt_lines.start, span.prompt_lines.end]))
      if (spans.length === 0) return null
      return spans
        .map((span) => `← prompt ${span.prompt_lines.start}–${span.prompt_lines.end}\n${span.note}`)
        .join('\n\n')
    },
    [wasmReady, data, activeFile],
  )

  const onReplay = useCallback(() => {
    const engine = engineRef.current
    if (!engine) return
    if (engine.playing()) {
      engine.pause()
      return
    }
    void engine.play()
  }, [])

  if (!data) {
    return <div className="napl-playground__placeholder">Loading {name}…</div>
  }

  return (
    <div>
      <div className="napl-playground__toolbar">
        <strong>{data.module}</strong>
        <span className="napl-playground__status">
          {data.target} · {data.attribution.length} attributions · {data.mapl.length} machine notes ·
          gen {data.journal.map((entry) => `#${entry.gen}`).join(' ') || '—'}
        </span>
        <button
          type="button"
          className="napl-playground__gen"
          onClick={onReplay}
          disabled={!wasmReady || data.session.events.length === 0}
        >
          {snapshot?.playing ? 'Pause' : 'Replay gen'}
        </button>
        {data.files.length > 1 ? (
          <select
            className="napl-playground__ghost"
            value={fileIndex}
            onChange={(event) => setFileIndex(Number(event.target.value))}
          >
            {data.files.map((file, index) => (
              <option key={file.path} value={index}>
                {file.path}
              </option>
            ))}
          </select>
        ) : null}
      </div>
      <div className="napl-showcase__panes">
        <div>
          <div className="napl-margin__label">prompt · {data.prompt.file}</div>
          <NaplEditor
            value={data.prompt.content}
            readOnly
            hover={promptHover}
            highlightRanges={promptHighlight}
          />
        </div>
        <div>
          <div className="napl-margin__label">generated · {activeFile?.path}</div>
          <NaplEditor
            key={activeFile?.path}
            value={generatedContent}
            readOnly
            hover={generatedHover}
            highlightRanges={genHighlight}
          />
        </div>
      </div>
      {data.mapl.length > 0 ? (
        <div className="napl-margin__section" style={{marginTop: '16px'}}>
          <div className="napl-margin__label">Machine layer · {data.module}.mapl</div>
          {data.mapl.map((entry, index) => (
            <div
              key={`${entry.kind}-${index}`}
              className="napl-margin__entry"
              data-severity={entry.severity}
            >
              <div className="napl-margin__kind">
                {entry.kind} · prompt {entry.promptLines[0]}–{entry.promptLines[1]}
              </div>
              <div>{entry.message}</div>
            </div>
          ))}
        </div>
      ) : null}
    </div>
  )
}
