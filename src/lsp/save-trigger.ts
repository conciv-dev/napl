export interface TargetTriggerState {
  target: string;
  currentPromptHash: string;
  promptHashAtGen: string | undefined;
}

export interface TriggerDecisionInput {
  enabled: boolean;
  module: string;
  targets: TargetTriggerState[];
}

export interface TriggerAction {
  module: string;
  target: string;
}

export function decideTriggers(input: TriggerDecisionInput): TriggerAction[] {
  if (!input.enabled) return [];
  const actions: TriggerAction[] = [];
  for (const state of input.targets) {
    if (state.currentPromptHash !== state.promptHashAtGen) {
      actions.push({ module: input.module, target: state.target });
    }
  }
  return actions;
}

export type ModulePhase = 'idle' | 'debouncing' | 'running' | 'queued';

export interface ModuleState {
  phase: ModulePhase;
}

export type SaveEvent =
  | { type: 'save' }
  | { type: 'debounceElapsed' }
  | { type: 'runFinished' };

export interface SaveEffects {
  startDebounce?: true;
  startRun?: true;
}

export interface ReduceResult {
  state: ModuleState;
  effects: SaveEffects;
}

export function initialModuleState(): ModuleState {
  return { phase: 'idle' };
}

export function reduceSave(state: ModuleState, event: SaveEvent): ReduceResult {
  switch (state.phase) {
    case 'idle':
      if (event.type === 'save') return { state: { phase: 'debouncing' }, effects: { startDebounce: true } };
      return { state, effects: {} };
    case 'debouncing':
      if (event.type === 'save') return { state: { phase: 'debouncing' }, effects: { startDebounce: true } };
      if (event.type === 'debounceElapsed') return { state: { phase: 'running' }, effects: { startRun: true } };
      return { state, effects: {} };
    case 'running':
      if (event.type === 'save') return { state: { phase: 'queued' }, effects: {} };
      if (event.type === 'runFinished') return { state: { phase: 'idle' }, effects: {} };
      return { state, effects: {} };
    case 'queued':
      if (event.type === 'save') return { state: { phase: 'queued' }, effects: {} };
      if (event.type === 'runFinished') return { state: { phase: 'running' }, effects: { startRun: true } };
      return { state, effects: {} };
    default:
      return { state, effects: {} };
  }
}

export interface SpawnCommand {
  command: string;
  args: string[];
}

export function buildSpawnCommand(
  cliPath: string,
  execPath: string,
  target: string,
  module: string,
): SpawnCommand {
  const genArgs = ['gen', target, '--module', module];
  if (cliPath.endsWith('.js') || cliPath.endsWith('.mjs') || cliPath.endsWith('.cjs')) {
    return { command: execPath, args: [cliPath, ...genArgs] };
  }
  return { command: cliPath, args: genArgs };
}
