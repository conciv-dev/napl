import { describe, expect, it } from 'vitest';
import {
  createReplayEngine,
  projectState,
  type ReplaySnapshot,
} from '../src/replay-engine.ts';
import type { RecordedSession } from '../src/gen-engine.ts';

const session: RecordedSession = {
  task: 'Generate the rust target for gen_prompt_diff',
  files: { 'gen_prompt_diff.napl': '---\nmodule: gen_prompt_diff\n---\nbody\n' },
  events: [
    { type: 'task', task: 'Generate the rust target for gen_prompt_diff' },
    { type: 'diff', path: 'src/lib.rs', patch: '@@ -0,0 +1 @@\n+fn a() {}' },
    { type: 'file-edit', path: 'src/lib.rs', content: 'fn a() {}\n' },
    {
      type: 'attribution',
      module: 'gen_prompt_diff',
      target: 'rust',
      entries: [
        { promptLines: [4, 4], file: 'src/lib.rs', lines: [1, 1], note: 'fn a' },
      ],
    },
    {
      type: 'mapl-entry',
      path: 'gen_prompt_diff.mapl',
      entry: {
        promptLines: [4, 4],
        kind: 'assumption',
        severity: 'warning',
        message: 'assumed no deps',
      },
    },
    {
      type: 'lock',
      module: 'gen_prompt_diff',
      target: 'rust',
      files: [{ path: 'src/lib.rs', hash: 'abc' }],
    },
  ],
};

describe('projectState', () => {
  it('starts from the session files with nothing applied at position 0', () => {
    const state = projectState(session, 0);
    expect(state.task).toBeNull();
    expect(state.files).toEqual(session.files);
    expect(state.locked).toBe(false);
    expect(state.attributions).toHaveLength(0);
  });

  it('accumulates events up to the given position', () => {
    const state = projectState(session, 4);
    expect(state.task).toBe(session.task);
    expect(state.patches['src/lib.rs']).toContain('+fn a');
    expect(state.files['src/lib.rs']).toBe('fn a() {}\n');
    expect(state.attributions).toHaveLength(1);
    expect(state.locked).toBe(false);
  });

  it('reflects the full session at the final position', () => {
    const state = projectState(session, session.events.length);
    expect(state.maplEntries).toHaveLength(1);
    expect(state.locks).toHaveLength(1);
    expect(state.locked).toBe(true);
  });

  it('clamps positions outside the event range', () => {
    expect(projectState(session, -5).task).toBeNull();
    expect(projectState(session, 999).locked).toBe(true);
  });
});

describe('createReplayEngine run()', () => {
  it('streams every event in order under instant mode', async () => {
    const engine = createReplayEngine(session, { instant: true });
    const seen: string[] = [];
    for await (const event of engine.run()) {
      seen.push(event.type);
    }
    expect(seen).toEqual(session.events.map((event) => event.type));
    expect(engine.done()).toBe(true);
    expect(engine.position()).toBe(session.events.length);
  });

  it('waits between events using the injected clock and honours speed', async () => {
    const waited: number[] = [];
    const engine = createReplayEngine(session, {
      speed: 2,
      wait: async (ms) => {
        waited.push(ms);
      },
    });
    for await (const _event of engine.run()) {
      void _event;
    }
    expect(waited).toHaveLength(session.events.length);
    expect(waited[0]).toBe(450 / 2);
    expect(waited.every((ms) => ms > 0)).toBe(true);
  });

  it('uses a custom per-event delay function when provided', async () => {
    const waited: number[] = [];
    const engine = createReplayEngine(session, {
      delayForEvent: (event) => (event.type === 'lock' ? 1000 : 10),
      wait: async (ms) => {
        waited.push(ms);
      },
    });
    for await (const _event of engine.run()) {
      void _event;
    }
    expect(waited.at(-1)).toBe(1000);
    expect(waited[0]).toBe(10);
  });
});

describe('createReplayEngine playback controls', () => {
  it('plays to completion and emits accumulating snapshots', async () => {
    const engine = createReplayEngine(session, { instant: true });
    const snapshots: ReplaySnapshot[] = [];
    engine.subscribe((snapshot) => snapshots.push(snapshot));
    await engine.play();
    expect(engine.done()).toBe(true);
    expect(engine.playing()).toBe(false);
    const final = snapshots.at(-1);
    expect(final?.state.locked).toBe(true);
    expect(final?.position).toBe(session.events.length);
  });

  it('pauses mid-run and resumes from the same position', async () => {
    let ticks = 0;
    const engine = createReplayEngine(session, {
      wait: async () => {
        ticks += 1;
        if (ticks === 2) {
          engine.pause();
        }
      },
    });
    await engine.play();
    expect(engine.playing()).toBe(false);
    expect(engine.position()).toBeLessThan(session.events.length);
    const halted = engine.position();
    await engine.play();
    expect(engine.position()).toBe(session.events.length);
    expect(engine.position()).toBeGreaterThan(halted);
  });

  it('seeks to an arbitrary position and reprojects state', () => {
    const engine = createReplayEngine(session, { instant: true });
    engine.seek(4);
    expect(engine.position()).toBe(4);
    expect(engine.snapshot().state.attributions).toHaveLength(1);
    engine.seek(1);
    expect(engine.snapshot().state.task).toBe(session.task);
    expect(engine.snapshot().state.attributions).toHaveLength(0);
  });

  it('resets back to the start', () => {
    const engine = createReplayEngine(session, { instant: true });
    engine.seek(session.events.length);
    engine.reset();
    expect(engine.position()).toBe(0);
    expect(engine.snapshot().state.locked).toBe(false);
  });

  it('delivers the current snapshot to a new subscriber immediately', () => {
    const engine = createReplayEngine(session, { instant: true });
    engine.seek(2);
    let received: ReplaySnapshot | null = null;
    engine.subscribe((snapshot) => {
      received = snapshot;
    });
    expect(received).not.toBeNull();
    expect((received as unknown as ReplaySnapshot).position).toBe(2);
  });
});
