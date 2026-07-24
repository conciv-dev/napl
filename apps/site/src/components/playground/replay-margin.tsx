import type {ProjectedState} from '@napl/editor'
import type {ReactElement} from 'react'

export interface ReplayMarginProps {
  state: ProjectedState | null
  compact?: boolean
}

export function ReplayMargin({state, compact = false}: ReplayMarginProps): ReactElement {
  if (!state || (!state.task && state.maplEntries.length === 0 && !state.locked)) {
    return <div className="napl-playground__placeholder">Run gen to replay this module.</div>
  }
  return (
    <div>
      {state.task ? (
        <div className="napl-margin__section">
          <div className="napl-margin__label">Task</div>
          <div className="napl-margin__task">{state.task}</div>
        </div>
      ) : null}
      {!compact && state.maplEntries.length > 0 ? (
        <div className="napl-margin__section">
          <div className="napl-margin__label">Machine layer</div>
          {state.maplEntries.map((item, index) => (
            <div
              key={`${item.entry.kind}-${index}`}
              className="napl-margin__entry"
              data-severity={item.entry.severity}
            >
              <div className="napl-margin__kind">
                {item.entry.kind} · prompt {item.entry.promptLines[0]}–{item.entry.promptLines[1]}
              </div>
              <div>{item.entry.message}</div>
            </div>
          ))}
        </div>
      ) : null}
      {state.locked ? (
        <div className="napl-margin__section">
          <span className="napl-margin__lock">
            ● locked {state.locks[0]?.files.length ?? 0} file
            {(state.locks[0]?.files.length ?? 0) === 1 ? '' : 's'} · attribution proven
          </span>
        </div>
      ) : null}
    </div>
  )
}
