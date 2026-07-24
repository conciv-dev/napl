import {
  useEffect,
  useRef,
  useState,
  type ComponentType,
  type ReactElement,
  type ReactNode,
} from 'react'
import {DynamicCodeBlock} from 'fumadocs-ui/components/dynamic-codeblock'
import type {InlineFile, NaplExampleImplProps} from './napl-example-impl'

const shikiLangForFilename = (name: string): string => {
  const lower = name.toLowerCase()
  if (lower.endsWith('.ts') || lower.endsWith('.tsx')) return 'typescript'
  if (lower.endsWith('.js') || lower.endsWith('.jsx') || lower.endsWith('.mjs') || lower.endsWith('.cjs'))
    return 'javascript'
  if (lower.endsWith('.json')) return 'json'
  if (lower.endsWith('.toml')) return 'toml'
  if (lower.endsWith('.rs')) return 'rust'
  return 'yaml'
}

export interface NaplExampleProps {
  module?: string
  files?: InlineFile[]
  code?: string
  children?: ReactNode
  filename?: string
  compact?: boolean
  readOnly?: boolean
  showGen?: boolean
}

type Impl = ComponentType<NaplExampleImplProps>

const toText = (value: string): string => value.replace(/\n$/, '')

const resolveFiles = (props: NaplExampleProps): InlineFile[] | undefined => {
  if (props.files) return props.files
  const raw = typeof props.code === 'string' ? props.code : typeof props.children === 'string' ? props.children : null
  if (raw === null) return undefined
  return [{name: props.filename ?? 'example.napl', content: toText(raw)}]
}

export function NaplExample(props: NaplExampleProps): ReactElement {
  const {module, filename} = props
  const inlineFiles = resolveFiles(props)
  const mountRef = useRef<HTMLDivElement | null>(null)
  const [Client, setClient] = useState<Impl | null>(null)

  useEffect(() => {
    const element = mountRef.current
    if (!element) return undefined
    const load = () => import('./napl-example-impl').then((m) => setClient(() => m.NaplExampleClient))
    if (typeof IntersectionObserver === 'undefined') {
      load()
      return undefined
    }
    const observer = new IntersectionObserver(
      (entries) => {
        if (entries.some((entry) => entry.isIntersecting)) {
          observer.disconnect()
          load()
        }
      },
      {rootMargin: '240px'},
    )
    observer.observe(element)
    return () => observer.disconnect()
  }, [])

  const fallbackSource = inlineFiles?.[0]?.content ?? ''

  return (
    <div ref={mountRef} className="napl-example-mount" data-napl-example={module ?? filename ?? 'snippet'}>
      {Client ? (
        <Client
          module={module}
          files={inlineFiles}
          compact={props.compact ?? true}
          readOnly={props.readOnly ?? false}
          showGen={props.showGen ?? false}
        />
      ) : (
        fallbackSource ? (
          <DynamicCodeBlock
            lang={shikiLangForFilename(filename ?? inlineFiles?.[0]?.name ?? 'example.napl')}
            code={fallbackSource}
            codeblock={{title: filename ?? inlineFiles?.[0]?.name, className: 'napl-example__fallback my-0'}}
          />
        ) : (
          <div className="napl-example napl-example--static">
            <div className="napl-example__placeholder">Loading example…</div>
          </div>
        )
      )}
    </div>
  )
}
