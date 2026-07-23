import { describe, expect, it } from 'vitest';
import {
  buildSpawnCommand,
  decideTriggers,
  initialModuleState,
  reduceSave,
} from '../src/lsp/save-trigger.js';
import type { ModuleState, SaveEvent } from '../src/lsp/save-trigger.js';

describe('decideTriggers', () => {
  it('triggers a target whose current prompt hash differs from promptHashAtGen', () => {
    const actions = decideTriggers({
      enabled: true,
      module: 'greeting',
      targets: [
        { target: 'typescript', currentPromptHash: 'new', promptHashAtGen: 'old' },
        { target: 'react', currentPromptHash: 'same', promptHashAtGen: 'same' },
      ],
    });
    expect(actions).toEqual([{ module: 'greeting', target: 'typescript' }]);
  });

  it('triggers a never-generated target (undefined promptHashAtGen)', () => {
    const actions = decideTriggers({
      enabled: true,
      module: 'greeting',
      targets: [{ target: 'typescript', currentPromptHash: 'h', promptHashAtGen: undefined }],
    });
    expect(actions).toEqual([{ module: 'greeting', target: 'typescript' }]);
  });

  it('never triggers when gen-on-save is disabled', () => {
    const actions = decideTriggers({
      enabled: false,
      module: 'greeting',
      targets: [{ target: 'typescript', currentPromptHash: 'new', promptHashAtGen: 'old' }],
    });
    expect(actions).toEqual([]);
  });
});

function drive(events: SaveEvent[]): { state: ModuleState; effects: ReturnType<typeof reduceSave>['effects'][] } {
  let state = initialModuleState();
  const effects: ReturnType<typeof reduceSave>['effects'][] = [];
  for (const event of events) {
    const result = reduceSave(state, event);
    state = result.state;
    effects.push(result.effects);
  }
  return { state, effects };
}

describe('reduceSave debounce collapse', () => {
  it('a save from idle starts a debounce', () => {
    const { state, effects } = drive([{ type: 'save' }]);
    expect(state.phase).toBe('debouncing');
    expect(effects[0]).toEqual({ startDebounce: true });
  });

  it('a newer save restarts the debounce rather than queueing a second run', () => {
    const { state, effects } = drive([{ type: 'save' }, { type: 'save' }, { type: 'save' }]);
    expect(state.phase).toBe('debouncing');
    expect(effects.every((effect) => effect.startDebounce === true)).toBe(true);
    expect(effects.some((effect) => effect.startRun === true)).toBe(false);
  });

  it('the debounce elapsing starts exactly one run', () => {
    const { state, effects } = drive([{ type: 'save' }, { type: 'debounceElapsed' }]);
    expect(state.phase).toBe('running');
    expect(effects[1]).toEqual({ startRun: true });
  });
});

describe('reduceSave queue-one semantics', () => {
  it('saves during a running gen collapse into a single queued follow-up', () => {
    const { state, effects } = drive([
      { type: 'save' },
      { type: 'debounceElapsed' },
      { type: 'save' },
      { type: 'save' },
    ]);
    expect(state.phase).toBe('queued');
    const runStarts = effects.filter((effect) => effect.startRun === true).length;
    expect(runStarts).toBe(1);
  });

  it('finishing a queued run starts exactly one follow-up run', () => {
    const { state, effects } = drive([
      { type: 'save' },
      { type: 'debounceElapsed' },
      { type: 'save' },
      { type: 'runFinished' },
    ]);
    expect(state.phase).toBe('running');
    expect(effects.filter((effect) => effect.startRun === true).length).toBe(2);
  });

  it('finishing a run with no queued save returns to idle', () => {
    const { state } = drive([{ type: 'save' }, { type: 'debounceElapsed' }, { type: 'runFinished' }]);
    expect(state.phase).toBe('idle');
  });
});

describe('buildSpawnCommand', () => {
  it('runs a bare command on PATH directly', () => {
    expect(buildSpawnCommand('hl', '/usr/bin/node', 'typescript', 'greeting')).toEqual({
      command: 'hl',
      args: ['gen', 'typescript', '--module', 'greeting'],
    });
  });

  it('runs a .js CLI entry via the node executable', () => {
    expect(buildSpawnCommand('/repo/dist/cli.js', '/usr/bin/node', 'typescript', 'greeting')).toEqual({
      command: '/usr/bin/node',
      args: ['/repo/dist/cli.js', 'gen', 'typescript', '--module', 'greeting'],
    });
  });
});
