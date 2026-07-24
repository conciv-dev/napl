import type {
  AttributionEntryEvent,
  GenEngine,
  GenEvent,
  LockedFile,
  MaplEntryEvent,
  RecordedSession,
} from './gen-engine.ts';

export interface ProjectedAttribution {
  module: string;
  target: string;
  entries: AttributionEntryEvent[];
}

export interface ProjectedMapl {
  path: string;
  entry: MaplEntryEvent;
}

export interface ProjectedLock {
  module: string;
  target: string;
  files: LockedFile[];
}

export interface ProjectedState {
  task: string | null;
  files: Record<string, string>;
  patches: Record<string, string>;
  attributions: ProjectedAttribution[];
  maplEntries: ProjectedMapl[];
  locks: ProjectedLock[];
  locked: boolean;
  error: string | null;
}

export interface ReplaySnapshot {
  position: number;
  length: number;
  playing: boolean;
  done: boolean;
  last: GenEvent | null;
  state: ProjectedState;
}

export interface ReplayOptions {
  speed?: number;
  instant?: boolean;
  baseDelayMs?: number;
  delayForEvent?: (event: GenEvent) => number;
  wait?: (ms: number) => Promise<void>;
}

export interface ReplayEngine extends GenEngine {
  readonly length: number;
  position(): number;
  playing(): boolean;
  done(): boolean;
  snapshot(): ReplaySnapshot;
  run(task?: string, files?: Record<string, string>): AsyncIterable<GenEvent>;
  play(): Promise<void>;
  pause(): void;
  toggle(): Promise<void>;
  seek(position: number): void;
  reset(): void;
  subscribe(listener: (snapshot: ReplaySnapshot) => void): () => void;
}

const DEFAULT_DELAYS: Record<GenEvent['type'], number> = {
  task: 450,
  'file-edit': 240,
  diff: 260,
  attribution: 340,
  'mapl-entry': 300,
  lock: 520,
  error: 320,
};

const clampPosition = (value: number, length: number): number =>
  Math.max(0, Math.min(Math.trunc(value), length));

export const projectState = (
  session: RecordedSession,
  position: number,
): ProjectedState => {
  const upto = clampPosition(position, session.events.length);
  const state: ProjectedState = {
    task: null,
    files: { ...session.files },
    patches: {},
    attributions: [],
    maplEntries: [],
    locks: [],
    locked: false,
    error: null,
  };
  for (let index = 0; index < upto; index += 1) {
    const event = session.events[index];
    if (!event) {
      continue;
    }
    switch (event.type) {
      case 'task':
        state.task = event.task;
        break;
      case 'file-edit':
        state.files = { ...state.files, [event.path]: event.content };
        break;
      case 'diff':
        state.patches = { ...state.patches, [event.path]: event.patch };
        break;
      case 'attribution':
        state.attributions = [
          ...state.attributions,
          { module: event.module, target: event.target, entries: event.entries },
        ];
        break;
      case 'mapl-entry':
        state.maplEntries = [
          ...state.maplEntries,
          { path: event.path, entry: event.entry },
        ];
        break;
      case 'lock':
        state.locks = [
          ...state.locks,
          { module: event.module, target: event.target, files: event.files },
        ];
        state.locked = true;
        break;
      case 'error':
        state.error = event.message;
        break;
    }
  }
  return state;
};

const defaultWait = (ms: number): Promise<void> =>
  ms <= 0 ? Promise.resolve() : new Promise((resolve) => setTimeout(resolve, ms));

export const createReplayEngine = (
  session: RecordedSession,
  options: ReplayOptions = {},
): ReplayEngine => {
  const speed = options.speed && options.speed > 0 ? options.speed : 1;
  const instant = options.instant ?? false;
  const wait = options.wait ?? defaultWait;
  const length = session.events.length;
  const listeners = new Set<(snapshot: ReplaySnapshot) => void>();

  let position = 0;
  let playing = false;
  let draining = false;

  const delayFor = (event: GenEvent): number => {
    if (instant) {
      return 0;
    }
    if (options.delayForEvent) {
      return options.delayForEvent(event) / speed;
    }
    const base = options.baseDelayMs ?? DEFAULT_DELAYS[event.type];
    return base / speed;
  };

  const snapshot = (): ReplaySnapshot => ({
    position,
    length,
    playing,
    done: position >= length,
    last: position > 0 ? session.events[position - 1] ?? null : null,
    state: projectState(session, position),
  });

  const notify = (): void => {
    const current = snapshot();
    for (const listener of listeners) {
      listener(current);
    }
  };

  const advance = (): GenEvent | null => {
    const event = session.events[position];
    if (!event) {
      return null;
    }
    position += 1;
    notify();
    return event;
  };

  async function* run(): AsyncIterable<GenEvent> {
    while (position < length) {
      const next = session.events[position];
      if (!next) {
        break;
      }
      await wait(delayFor(next));
      const event = advance();
      if (!event) {
        break;
      }
      yield event;
    }
  }

  const play = async (): Promise<void> => {
    playing = true;
    if (draining) {
      notify();
      return;
    }
    if (position >= length) {
      position = 0;
    }
    draining = true;
    notify();
    try {
      while (playing && position < length) {
        const next = session.events[position];
        if (!next) {
          break;
        }
        await wait(delayFor(next));
        if (!playing) {
          break;
        }
        advance();
      }
    } finally {
      playing = false;
      draining = false;
      notify();
    }
  };

  const pause = (): void => {
    playing = false;
    notify();
  };

  const toggle = async (): Promise<void> => {
    if (playing) {
      pause();
      return;
    }
    await play();
  };

  const seek = (next: number): void => {
    position = clampPosition(next, length);
    notify();
  };

  const reset = (): void => {
    seek(0);
  };

  const subscribe = (listener: (snapshot: ReplaySnapshot) => void): (() => void) => {
    listeners.add(listener);
    listener(snapshot());
    return () => {
      listeners.delete(listener);
    };
  };

  return {
    length,
    position: () => position,
    playing: () => playing,
    done: () => position >= length,
    snapshot,
    run,
    play,
    pause,
    toggle,
    seek,
    reset,
    subscribe,
  };
};
