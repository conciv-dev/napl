import {
  useEffect,
  useRef,
  useState,
  type ComponentType,
  type ReactElement,
  type ReactNode,
} from 'react'
import type {ShowcaseModule} from '@/lib/fixtures'
import type {NaplPlaygroundClientProps} from './napl-playground-impl'

export interface NaplPlaygroundProps {
  module: string
  compact?: boolean
  speed?: number
  fallback?: ReactNode
}

type ClientComponent = ComponentType<NaplPlaygroundClientProps>

export function NaplPlayground({
  module,
  compact = false,
  speed = 1,
  fallback,
}: NaplPlaygroundProps): ReactElement {
  const mountRef = useRef<HTMLDivElement | null>(null)
  const [visible, setVisible] = useState(false)
  const [Client, setClient] = useState<ClientComponent | null>(null)
  const [data, setData] = useState<ShowcaseModule | null>(null)

  useEffect(() => {
    const element = mountRef.current
    if (!element) return undefined
    if (typeof IntersectionObserver === 'undefined') {
      setVisible(true)
      return undefined
    }
    const observer = new IntersectionObserver(
      (entries) => {
        if (entries.some((entry) => entry.isIntersecting)) {
          setVisible(true)
          observer.disconnect()
        }
      },
      {rootMargin: '240px'},
    )
    observer.observe(element)
    return () => observer.disconnect()
  }, [])

  useEffect(() => {
    if (!visible) return undefined
    let active = true
    Promise.all([
      import('./napl-playground-impl'),
      import('@/lib/fixtures').then((fixtures) => fixtures.loadShowcaseModule(module)),
    ]).then(([clientModule, loaded]) => {
      if (!active) return
      setClient(() => clientModule.NaplPlaygroundClient)
      setData(loaded)
    })
    return () => {
      active = false
    }
  }, [visible, module])

  return (
    <div ref={mountRef} className="napl-playground-mount" data-playground-mount={module}>
      {Client && data ? (
        <Client module={data} compact={compact} speed={speed} />
      ) : (
        (fallback ?? <PlaygroundFallback module={module} />)
      )}
    </div>
  )
}

function PlaygroundFallback({module}: {module: string}): ReactElement {
  return (
    <div className="napl-playground" data-playground-fallback={module}>
      <div className="napl-playground__chrome">
        <div className="napl-playground__dots" aria-hidden="true">
          <span />
          <span />
          <span />
        </div>
        <span className="napl-playground__title">{module}.napl</span>
      </div>
      <div className="napl-playground__body">
        <div className="napl-playground__placeholder" style={{padding: '16px'}}>
          Loading interactive playground…
        </div>
      </div>
    </div>
  )
}
