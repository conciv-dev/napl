import {
  useEffect,
  useState,
  type ComponentType,
  type ReactElement,
} from 'react'
import {Group, Panel, Separator} from 'react-resizable-panels'
import {useTheme} from 'next-themes'
import {showcaseIndex} from '@/lib/fixtures'
import {ModuleTree} from './module-tree'
import type {ShowcaseDetailProps} from './showcase-detail-impl'

type DetailComponent = ComponentType<ShowcaseDetailProps>

export function ShowcaseBrowser(): ReactElement {
  const first = showcaseIndex[0]?.module ?? ''
  const [selected, setSelected] = useState(first)
  const [Detail, setDetail] = useState<DetailComponent | null>(null)
  const {resolvedTheme} = useTheme()
  const theme = resolvedTheme === 'light' ? 'light' : 'dark'

  useEffect(() => {
    let active = true
    import('./showcase-detail-impl').then((module) => {
      if (active) setDetail(() => module.ShowcaseDetailClient)
    })
    return () => {
      active = false
    }
  }, [])

  return (
    <div className="h-[calc(100dvh-9.5rem)] min-h-[540px] w-full overflow-hidden rounded-xl border border-fd-border bg-fd-card shadow-sm">
      <Group orientation="horizontal" className="h-full">
        <Panel defaultSize="20%" minSize="14%" maxSize="34%" className="min-w-0">
          <div data-testid="sidebar" className="flex h-full flex-col bg-fd-secondary/40">
            <div className="shrink-0 border-b border-fd-border px-3 py-2.5 text-[11px] font-semibold uppercase tracking-wider text-fd-muted-foreground">
              Modules · {showcaseIndex.length}
            </div>
            <div className="min-h-0 flex-1 overflow-auto p-2">
              <ModuleTree entries={showcaseIndex} selected={selected} onSelect={setSelected} />
            </div>
          </div>
        </Panel>
        <Separator className="w-1.5 shrink-0 cursor-col-resize bg-fd-border/70 outline-none transition-colors hover:bg-fd-primary data-[resize-handle-state=drag]:bg-fd-primary data-[resize-handle-state=hover]:bg-fd-primary" />
        <Panel defaultSize="80%" minSize="40%" className="min-w-0">
          {Detail ? (
            <Detail name={selected} theme={theme} />
          ) : (
            <div className="flex h-full items-center justify-center text-sm text-fd-muted-foreground">
              Loading workbench…
            </div>
          )}
        </Panel>
      </Group>
    </div>
  )
}
