import {
  useEffect,
  useRef,
  useState,
  type ComponentType,
  type ReactElement,
} from 'react'
import {showcaseIndex} from '@/lib/fixtures'
import type {ShowcaseDetailProps} from './showcase-detail-impl'

type DetailComponent = ComponentType<ShowcaseDetailProps>

export function ShowcaseBrowser(): ReactElement {
  const first = showcaseIndex[0]?.module ?? ''
  const [selected, setSelected] = useState(first)
  const [Detail, setDetail] = useState<DetailComponent | null>(null)
  const mounted = useRef(false)

  useEffect(() => {
    mounted.current = true
    import('./showcase-detail-impl').then((module) => {
      setDetail(() => module.ShowcaseDetailClient)
    })
  }, [])

  return (
    <div className="napl-showcase">
      <div className="napl-showcase__list" role="tablist" aria-label="Self-hosted modules">
        {showcaseIndex.map((entry) => (
          <button
            key={entry.module}
            type="button"
            role="tab"
            aria-selected={entry.module === selected}
            className={
              entry.module === selected
                ? 'napl-showcase__item napl-showcase__item--active'
                : 'napl-showcase__item'
            }
            onClick={() => setSelected(entry.module)}
          >
            {entry.module}
          </button>
        ))}
      </div>
      <div data-testid="showcase-detail">
        {Detail ? (
          <Detail name={selected} />
        ) : (
          <div className="napl-playground__placeholder">Loading module browser…</div>
        )}
      </div>
    </div>
  )
}
