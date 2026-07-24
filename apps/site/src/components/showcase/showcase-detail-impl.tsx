import {
  createReplayEngine,
  languageForFilename,
  NaplEditor,
  type EditorTheme,
  type HighlightRange,
  type HoverCard,
  type HoverContext,
  type NaplEditorApi,
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
import {Group, Panel, Separator} from 'react-resizable-panels'
import {
  Bot,
  CircleAlert,
  FileCode2,
  FileCog,
  FileJson,
  Info,
  Lock,
  MousePointerClick,
  Pause,
  Play,
  TriangleAlert,
  X,
} from 'lucide-react'
import {ensureNaplWasm} from '@/lib/napl-wasm.browser'
import {
  excerptFromContent,
  fileHighlightsFor,
  promptHighlightsFor,
  resolveGeneratedHover,
  resolvePromptHover,
} from '@/lib/attribution'
import {loadShowcaseModule} from '@/lib/fixtures'
import type {ShowcaseModule} from '@/lib/fixtures'
import {cn} from '@/lib/utils'
import {Button} from '@/components/ui/button'

export interface ShowcaseDetailProps {
  name: string
  theme: EditorTheme
}

const HINT_KEY = 'napl-showcase-hover-hint-dismissed'

const KIND_LABEL: Record<string, string> = {
  'no-op': 'No change needed',
  assumption: 'Assumption the agent made',
  ambiguity: 'Ambiguity in the prompt',
  note: 'Note from the agent',
}

const countLabel = (n: number): string => (n === 1 ? 'once' : n === 2 ? 'twice' : `${n} times`)

const fileIcon = (path: string) => {
  if (path.endsWith('.mapl')) return Bot
  if (path.endsWith('.json')) return FileJson
  if (path.endsWith('.toml')) return FileCog
  return FileCode2
}

const severityIcon = (severity: string) => {
  if (severity === 'error') return CircleAlert
  if (severity === 'warning') return TriangleAlert
  return Info
}

const severityClass = (severity: string) =>
  severity === 'error'
    ? 'text-red-500'
    : severity === 'warning'
      ? 'text-amber-500'
      : 'text-fd-primary'

export function ShowcaseDetailClient({name, theme}: ShowcaseDetailProps): ReactElement {
  const [wasmReady, setWasmReady] = useState(false)
  const [data, setData] = useState<ShowcaseModule | null>(null)
  const [fileIndex, setFileIndex] = useState(0)
  const [promptHighlight, setPromptHighlight] = useState<HighlightRange[]>([])
  const [genHighlight, setGenHighlight] = useState<HighlightRange[]>([])
  const [snapshot, setSnapshot] = useState<ReplaySnapshot | null>(null)
  const [hintDismissed, setHintDismissed] = useState(true)
  const engineRef = useRef<ReplayEngine | null>(null)
  const promptApiRef = useRef<NaplEditorApi | null>(null)
  const genApiRef = useRef<NaplEditorApi | null>(null)

  useEffect(() => {
    setHintDismissed(
      typeof window !== 'undefined' && window.localStorage.getItem(HINT_KEY) === '1',
    )
  }, [])

  const dismissHint = useCallback(() => {
    setHintDismissed(true)
    if (typeof window !== 'undefined') window.localStorage.setItem(HINT_KEY, '1')
  }, [])

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

  const viewFiles = useMemo(() => {
    if (!data) return []
    if (!data.maplContent) return data.files
    return [
      ...data.files,
      {path: data.maplFile, journalPath: '', content: data.maplContent, language: 'mapl'},
    ]
  }, [data])

  const activeFile = viewFiles[fileIndex] ?? null
  const isMaplView = Boolean(activeFile && data && activeFile.path === data.maplFile)

  const generatedContent = useMemo(() => {
    if (!activeFile) return ''
    if (isMaplView) return activeFile.content
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
    (context: HoverContext): HoverCard | null => {
      if (!wasmReady || !data?.attributionYaml || !activeFile) return null
      const spans = resolvePromptHover(
        data.prompt.content,
        data.attributionYaml,
        data.promptAtGen,
        context.line,
      )
      if (spans.length === 0) {
        setGenHighlight([])
        return null
      }
      const chosen = spans.find((span) => span.file === activeFile.path) ?? spans[0]
      if (!chosen) return null
      setPromptHighlight(promptHighlightsFor(spans))
      const activeLines = fileHighlightsFor(spans, activeFile.path)
      setGenHighlight(activeLines.length > 0 ? activeLines : [chosen.lines])
      const excerptFile = data.files.find((file) => file.path === chosen.file)
      return {
        kind: 'card',
        excerpt: excerptFile
          ? {code: excerptFromContent(excerptFile.content, chosen.lines), caption: chosen.note, language: languageForFilename(chosen.file)}
          : undefined,
        meta: `${chosen.file} · lines ${chosen.lines[0]}–${chosen.lines[1]}`,
        jump: {
          label: `Go to ${chosen.file}:${chosen.lines[0]} ↵`,
          onJump: () => {
            const index = data.files.findIndex((file) => file.path === chosen.file)
            if (index >= 0) setFileIndex(index)
            setGenHighlight([chosen.lines])
            requestAnimationFrame(() => genApiRef.current?.scrollToLine(chosen.lines[0]))
          },
        },
      }
    },
    [wasmReady, data, activeFile],
  )

  const generatedHover = useCallback(
    (context: HoverContext): HoverCard | null => {
      if (!wasmReady || !data?.attributionYaml || !activeFile) return null
      const spans = resolveGeneratedHover(
        data.prompt.content,
        data.attributionYaml,
        data.promptAtGen,
        activeFile.path,
        context.line,
      )
      const chosen = spans[0]
      if (!chosen) {
        setPromptHighlight([])
        return null
      }
      setPromptHighlight(promptHighlightsFor(spans))
      setGenHighlight([chosen.lines])
      return {
        kind: 'card',
        heading: 'Comes from the prompt',
        quote: chosen.sentence || undefined,
        meta: `${data.prompt.file} · lines ${chosen.promptLines[0]}–${chosen.promptLines[1]} · ${chosen.note}`,
        jump: {
          label: `Go to ${data.prompt.file}:${chosen.promptDocLines[0]} ↵`,
          onJump: () => {
            setPromptHighlight([chosen.promptDocLines])
            requestAnimationFrame(() => promptApiRef.current?.scrollToLine(chosen.promptDocLines[0]))
          },
        },
      }
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
    return (
      <div className="flex h-full items-center justify-center text-sm text-fd-muted-foreground">
        Loading {name}…
      </div>
    )
  }

  const playing = snapshot?.playing ?? false
  const genCount = data.journal.length
  const targetLabel = data.target.charAt(0).toUpperCase() + data.target.slice(1)
  const genHistory =
    data.journal
      .map(
        (_entry, index) =>
          `Generation ${index + 1} · ${index === 0 ? 'first gen from the prompt' : 'regenerated after a prompt change'}`,
      )
      .join('\n') || 'Generated from the prompt'

  return (
    <div className="dark flex h-full min-h-0 flex-col bg-fd-background text-fd-foreground" data-testid="showcase-detail">
      <div className="flex shrink-0 flex-wrap items-center gap-x-3 gap-y-1 border-b border-fd-border px-4 py-2.5">
        <div className="flex flex-col">
          <span
            data-testid="module-name"
            className="font-mono text-[15px] font-semibold text-fd-foreground"
          >
            {data.module}
          </span>
          <span className="text-xs text-fd-muted-foreground" title={genHistory}>
            {targetLabel} module · generated {genCount === 0 ? 'from the prompt' : countLabel(genCount)}
          </span>
        </div>
        {data.lock.length > 0 ? (
          <span
            className="hidden items-center gap-1 text-xs text-fd-muted-foreground sm:inline-flex"
            title={`${data.lock.length} generated file${data.lock.length === 1 ? '' : 's'}, locked read-only by the toolchain so you edit the prompt, not the output`}
          >
            <Lock className="size-3" /> read-only
          </span>
        ) : null}
        <div className="ml-auto flex items-center gap-1.5">
          {viewFiles.length > 1 ? (
            <div className="flex items-center gap-1 rounded-lg border border-fd-border bg-fd-card p-0.5">
              {viewFiles.map((file, index) => {
                const Icon = fileIcon(file.path)
                return (
                  <button
                    key={file.path}
                    type="button"
                    onClick={() => setFileIndex(index)}
                    className={cn(
                      'flex items-center gap-1 rounded-md px-2 py-1 font-mono text-xs transition-colors',
                      index === fileIndex
                        ? 'bg-fd-primary/10 font-medium text-fd-primary'
                        : 'text-fd-muted-foreground hover:bg-fd-accent hover:text-fd-foreground',
                    )}
                  >
                    <Icon className="size-3" />
                    <span className="max-w-[11rem] truncate">{file.path}</span>
                  </button>
                )
              })}
            </div>
          ) : null}
          <Button
            size="sm"
            data-testid="replay-gen"
            onClick={onReplay}
            disabled={!wasmReady || data.session.events.length === 0}
            title="Watch the agent regenerate this module from the prompt, step by step"
          >
            {playing ? <Pause className="size-3.5" /> : <Play className="size-3.5" />}
            {playing ? 'Pause' : 'Watch it generate'}
          </Button>
        </div>
      </div>

      <div className="min-h-0 flex-1">
        <Group orientation="horizontal" className="h-full">
          <Panel defaultSize="52%" minSize="28%" className="flex min-w-0 flex-col">
            <div className="flex shrink-0 items-center gap-1.5 border-b border-fd-border/70 bg-fd-secondary/60 px-4 py-1.5 text-[11px] font-medium text-fd-muted-foreground">
              <span className="uppercase tracking-wide text-fd-foreground/70">Prompt</span>
              <span className="font-mono lowercase">{data.prompt.file}</span>
            </div>
            <div className="min-h-0 flex-1 overflow-hidden" data-testid="prompt-pane">
              <NaplEditor
                value={data.prompt.content}
                readOnly
                language="napl"
                theme="dark"
                hover={promptHover}
                highlightRanges={promptHighlight}
                apiRef={promptApiRef}
              />
            </div>
          </Panel>
          <Separator className="w-1.5 shrink-0 cursor-col-resize bg-fd-border/70 outline-none transition-colors hover:bg-fd-primary data-[resize-handle-state=hover]:bg-fd-primary data-[resize-handle-state=drag]:bg-fd-primary" />
          <Panel defaultSize="48%" minSize="28%" className="flex min-w-0 flex-col">
            <div className="flex shrink-0 items-center gap-1.5 border-b border-fd-border/70 bg-fd-secondary/60 px-4 py-1.5 text-[11px] font-medium text-fd-muted-foreground">
              <span className="uppercase tracking-wide text-fd-foreground/70">
                {isMaplView ? 'Machine layer' : 'Generated'}
              </span>
              <span className="font-mono">{activeFile?.path}</span>
            </div>
            {!hintDismissed && !isMaplView ? (
              <div className="flex shrink-0 items-center gap-2 border-b border-fd-border/70 bg-fd-primary/5 px-4 py-1.5 text-xs text-fd-muted-foreground">
                <MousePointerClick className="size-3.5 shrink-0 text-fd-primary" />
                <span className="flex-1">
                  Hover any line to trace it back to the prompt sentence that produced it.
                </span>
                <button
                  type="button"
                  onClick={dismissHint}
                  aria-label="Dismiss hint"
                  className="rounded p-0.5 text-fd-muted-foreground hover:bg-fd-accent hover:text-fd-foreground"
                >
                  <X className="size-3.5" />
                </button>
              </div>
            ) : null}
            <div className="min-h-0 flex-1 overflow-hidden" data-testid="generated-pane">
              <NaplEditor
                key={activeFile?.path}
                value={generatedContent}
                readOnly
                language={isMaplView ? 'mapl' : activeFile ? languageForFilename(activeFile.path) : 'text'}
                theme="dark"
                hover={isMaplView ? undefined : generatedHover}
                highlightRanges={isMaplView ? [] : genHighlight}
                apiRef={genApiRef}
              />
            </div>
          </Panel>
        </Group>
      </div>

      {data.mapl.length > 0 ? (
        <details className="group/mapl shrink-0 border-t border-fd-border bg-fd-secondary/30" open>
          <summary className="flex cursor-pointer list-none items-center gap-2 px-4 py-2 text-xs text-fd-muted-foreground select-none marker:hidden">
            <span
              className="font-medium text-fd-foreground/70"
              title="Notes the AI agent left about its own work — assumptions it made, ambiguities in the prompt, and changes it decided were unnecessary."
            >
              Machine layer
            </span>
            <span className="text-fd-muted-foreground">
              {data.mapl.length} note{data.mapl.length === 1 ? '' : 's'}
            </span>
            <span className="ml-auto hidden items-center gap-3 text-[10px] sm:flex">
              <span className="inline-flex items-center gap-1">
                <CircleAlert className="size-3 text-red-500" /> needs attention
              </span>
              <span className="inline-flex items-center gap-1">
                <TriangleAlert className="size-3 text-amber-500" /> assumption
              </span>
              <span className="inline-flex items-center gap-1">
                <Info className="size-3 text-fd-primary" /> note
              </span>
            </span>
          </summary>
          <div className="max-h-40 overflow-auto px-4 pb-3">
            <div className="flex flex-col gap-2">
              {data.mapl.map((entry, index) => {
                const Icon = severityIcon(entry.severity)
                return (
                  <div key={`${entry.kind}-${index}`} className="flex gap-2 text-xs leading-snug">
                    <Icon className={cn('mt-0.5 size-3.5 shrink-0', severityClass(entry.severity))} />
                    <div>
                      <div className="text-[11px] font-medium text-fd-foreground/70">
                        {KIND_LABEL[entry.kind] ?? entry.kind} · prompt lines {entry.promptLines[0]}–
                        {entry.promptLines[1]}
                      </div>
                      <div className="text-fd-muted-foreground">{entry.message}</div>
                    </div>
                  </div>
                )
              })}
            </div>
          </div>
        </details>
      ) : null}
    </div>
  )
}
