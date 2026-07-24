import type {ProjectedState} from '@napl/editor'
import type {ReactElement} from 'react'

export interface ReplayMarginProps {
  state: ProjectedState | null
  compact?: boolean
}

const KIND_LABEL: Record<string, string> = {
  'no-op': 'No change needed',
  assumption: 'Assumption the agent made',
  ambiguity: 'Ambiguity in the prompt',
  note: 'Note from the agent',
}

export function ReplayMargin({state, compact = false}: ReplayMarginProps): ReactElement {
  if (!state || (!state.task && state.maplEntries.length === 0 && !state.locked)) {
    return (
      <div className="napl-playground__placeholder">
        Press Generate to watch the agent build this module from the prompt.
      </div>
    )
  }
  const lockedCount = state.locks[0]?.files.length ?? 0
  return (
    <div>
      {state.task ? (
        <div className="napl-margin__section">
          <div className="napl-margin__label">The task handed to the agent</div>
          <div className="napl-margin__task">{state.task}</div>
        </div>
      ) : null}
      {!compact && state.maplEntries.length > 0 ? (
        <div className="napl-margin__section">
          <div
            className="napl-margin__label"
            title="Notes the AI agent left about its own work — assumptions, ambiguities, and no-ops."
          >
            Machine layer
          </div>
          {state.maplEntries.map((item, index) => (
            <div
              key={`${item.entry.kind}-${index}`}
              className="napl-margin__entry"
              data-severity={item.entry.severity}
            >
              <div className="napl-margin__kind">
                {KIND_LABEL[item.entry.kind] ?? item.entry.kind} · prompt lines{' '}
                {item.entry.promptLines[0]}–{item.entry.promptLines[1]}
              </div>
              <div>{item.entry.message}</div>
            </div>
          ))}
        </div>
      ) : null}
      {state.locked ? (
        <div className="napl-margin__section">
          <span className="napl-margin__lock">
            ● {lockedCount} file{lockedCount === 1 ? '' : 's'} generated and locked read-only
          </span>
        </div>
      ) : null}
    </div>
  )
}
