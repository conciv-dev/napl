import {useMemo, type ReactElement} from 'react'
import {Accordion} from 'radix-ui'
import {ChevronRight, FileText, Folder} from 'lucide-react'
import {cn} from '@/lib/utils'
import type {ShowcaseIndexEntry} from '@/lib/fixtures'

export interface ModuleTreeProps {
  entries: ShowcaseIndexEntry[]
  selected: string
  onSelect: (module: string) => void
}

const DIR_ORDER = ['schemas', 'core', 'cli', 'root']
const DIR_LABEL: Record<string, string> = {
  schemas: 'schemas',
  core: 'core',
  cli: 'cli',
  root: 'modules',
}

export function ModuleTree({entries, selected, onSelect}: ModuleTreeProps): ReactElement {
  const groups = useMemo(() => {
    const byDir = new Map<string, ShowcaseIndexEntry[]>()
    for (const entry of entries) {
      const dir = entry.dir || 'root'
      const bucket = byDir.get(dir) ?? []
      bucket.push(entry)
      byDir.set(dir, bucket)
    }
    return [...byDir.entries()]
      .sort((a, b) => {
        const ai = DIR_ORDER.indexOf(a[0])
        const bi = DIR_ORDER.indexOf(b[0])
        return (ai < 0 ? 99 : ai) - (bi < 0 ? 99 : bi)
      })
      .map(([dir, list]) => ({
        dir,
        list: [...list].sort((a, b) => a.module.localeCompare(b.module)),
      }))
  }, [entries])

  return (
    <Accordion.Root
      type="multiple"
      defaultValue={groups.map((group) => group.dir)}
      className="flex flex-col gap-0.5"
    >
      {groups.map((group) => (
        <Accordion.Item key={group.dir} value={group.dir} className="border-none">
          <Accordion.Header className="m-0">
            <Accordion.Trigger
              data-testid={`tree-folder-${group.dir}`}
              className="group/dir flex w-full items-center gap-1.5 rounded-md px-2 py-1.5 text-left text-xs font-medium text-fd-muted-foreground transition-colors outline-none hover:bg-fd-accent hover:text-fd-foreground focus-visible:ring-2 focus-visible:ring-fd-ring/60"
            >
              <ChevronRight className="size-3.5 shrink-0 transition-transform duration-200 group-data-[state=open]/dir:rotate-90" />
              <Folder className="size-3.5 shrink-0 text-fd-muted-foreground" />
              <span className="flex-1">{DIR_LABEL[group.dir] ?? group.dir}/</span>
              <span className="rounded-full bg-fd-secondary px-1.5 py-0.5 text-[10px] tabular-nums text-fd-muted-foreground">
                {group.list.length}
              </span>
            </Accordion.Trigger>
          </Accordion.Header>
          <Accordion.Content className="overflow-hidden">
            <div className="ml-2 flex flex-col gap-0.5 border-l border-fd-border/70 pt-0.5 pl-1.5">
              {group.list.map((entry) => {
                const active = entry.module === selected
                return (
                  <button
                    key={entry.module}
                    type="button"
                    data-testid={`tree-file-${entry.module}`}
                    aria-current={active ? 'true' : undefined}
                    onClick={() => onSelect(entry.module)}
                    className={cn(
                      'flex items-center gap-1.5 rounded-md px-2 py-1 text-left text-[13px] transition-colors outline-none focus-visible:ring-2 focus-visible:ring-fd-ring/60',
                      active
                        ? 'bg-fd-primary/10 font-medium text-fd-primary'
                        : 'text-fd-foreground/80 hover:bg-fd-accent hover:text-fd-foreground',
                    )}
                  >
                    <FileText
                      className={cn(
                        'size-3.5 shrink-0',
                        active ? 'text-fd-primary' : 'text-fd-muted-foreground/70',
                      )}
                    />
                    <span className="truncate font-mono text-xs">{entry.promptFile}</span>
                  </button>
                )
              })}
            </div>
          </Accordion.Content>
        </Accordion.Item>
      ))}
    </Accordion.Root>
  )
}
