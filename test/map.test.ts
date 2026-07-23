import { mkdtemp, readFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import {
  declaredTargetsForModule,
  emptyMap,
  filesForModule,
  hasModule,
  isPromptGenStale,
  parseMap,
  readMap,
  recordAttribution,
  recordUnattributed,
  writeMap,
} from '../src/core/map.js';
import type { HlMap } from '../src/core/map.js';

let dir: string;

beforeEach(async () => {
  dir = await mkdtemp(join(tmpdir(), 'hl-map-'));
});

afterEach(async () => {
  await rm(dir, { recursive: true, force: true });
});

function seeded(): HlMap {
  const map = emptyMap();
  recordAttribution(map, {
    rel: 'examples/greeting.hl',
    module: 'greeting',
    promptHash: 'p1',
    target: 'typescript',
    declaredTargets: ['typescript'],
    files: [
      { filePath: '.hl/src/typescript/greeting.ts', hash: 'h1' },
      { filePath: '.hl/src/typescript/greeting.test.ts', hash: 'h2' },
    ],
  });
  return map;
}

describe('map read/write', () => {
  it('returns an empty v2 map when the file is absent', async () => {
    const map = await readMap(join(dir, 'missing.json'));
    expect(map).toEqual(emptyMap());
    expect(map.version).toBe(2);
  });

  it('round-trips a written map', async () => {
    const mapPath = join(dir, '.hl', 'map.json');
    const map = seeded();
    await writeMap(mapPath, map);
    const roundTrip = await readMap(mapPath);
    expect(roundTrip).toEqual(map);
    const raw = await readFile(mapPath, 'utf8');
    expect(raw.endsWith('\n')).toBe(true);
  });

  it('throws on corrupt json', () => {
    expect(() => parseMap('{ not json')).toThrow(/corrupt map.json/);
  });

  it('applies defaults for sparse prompt records', () => {
    const map = parseMap(
      JSON.stringify({
        version: 2,
        prompts: { 'a.hl': { module: 'm', promptHash: 'h' } },
      }),
    );
    expect(map.prompts['a.hl'].declaredTargets).toEqual([]);
    expect(map.prompts['a.hl'].targets).toEqual({});
  });
});

describe('map query helpers', () => {
  it('resolves module membership, targets, and files', () => {
    const map = seeded();
    expect(hasModule(map, 'greeting')).toBe(true);
    expect(hasModule(map, 'missing')).toBe(false);
    expect(declaredTargetsForModule(map, 'greeting')).toEqual(['typescript']);
    const files = filesForModule(map, 'greeting');
    expect(files).toEqual([
      { target: 'typescript', filePath: '.hl/src/typescript/greeting.ts' },
      { target: 'typescript', filePath: '.hl/src/typescript/greeting.test.ts' },
    ]);
  });

  it('records file-to-prompt attribution as many-to-many', () => {
    const map = seeded();
    recordAttribution(map, {
      rel: 'examples/extra.hl',
      module: 'extra',
      promptHash: 'p2',
      target: 'typescript',
      declaredTargets: ['typescript'],
      files: [{ filePath: '.hl/src/typescript/greeting.ts', hash: 'h1' }],
    });
    expect(map.files['.hl/src/typescript/greeting.ts'].prompts.sort()).toEqual([
      'examples/extra.hl',
      'examples/greeting.hl',
    ]);
  });

  it('drops orphaned file attributions when a prompt no longer produces them', () => {
    const map = seeded();
    recordAttribution(map, {
      rel: 'examples/greeting.hl',
      module: 'greeting',
      promptHash: 'p1b',
      target: 'typescript',
      declaredTargets: ['typescript'],
      files: [{ filePath: '.hl/src/typescript/greeting.ts', hash: 'h1b' }],
    });
    expect(map.files['.hl/src/typescript/greeting.test.ts']).toBeUndefined();
    expect(map.prompts['examples/greeting.hl'].targets.typescript.files).toEqual([
      '.hl/src/typescript/greeting.ts',
    ]);
  });
});

describe('isPromptGenStale', () => {
  it('is not stale when the prompt hash matches and not forced', () => {
    const map = seeded();
    expect(isPromptGenStale(map.prompts['examples/greeting.hl'], 'typescript', 'p1', false)).toBe(false);
  });

  it('is stale when the prompt hash changed', () => {
    const map = seeded();
    expect(isPromptGenStale(map.prompts['examples/greeting.hl'], 'typescript', 'p2', false)).toBe(true);
  });

  it('is stale for an ungenerated target', () => {
    const map = seeded();
    expect(isPromptGenStale(map.prompts['examples/greeting.hl'], 'react', 'p1', false)).toBe(true);
  });

  it('is stale when there is no prior record', () => {
    expect(isPromptGenStale(undefined, 'typescript', 'p1', false)).toBe(true);
  });

  it('is stale when forced even if the hash matches', () => {
    const map = seeded();
    expect(isPromptGenStale(map.prompts['examples/greeting.hl'], 'typescript', 'p1', true)).toBe(true);
  });

  it('is stale when the target carries an unattributed marker', () => {
    const map = seeded();
    recordUnattributed(map, {
      rel: 'examples/greeting.hl',
      module: 'greeting',
      promptHash: 'p1',
      target: 'typescript',
      declaredTargets: ['typescript'],
      files: ['.hl/src/typescript/greeting.ts'],
    });
    expect(isPromptGenStale(map.prompts['examples/greeting.hl'], 'typescript', 'p1', false)).toBe(true);
  });
});

describe('recordUnattributed', () => {
  it('marks the target, drops the recorded prompt hash, keeps the file list, and detaches file records', () => {
    const map = seeded();
    recordUnattributed(map, {
      rel: 'examples/greeting.hl',
      module: 'greeting',
      promptHash: 'p1',
      target: 'typescript',
      declaredTargets: ['typescript'],
      files: ['.hl/src/typescript/greeting.ts'],
    });
    const entry = map.prompts['examples/greeting.hl'].targets.typescript;
    expect(entry.unattributed).toBe(true);
    expect(entry.promptHashAtGen).toBeUndefined();
    expect(entry.files).toEqual(['.hl/src/typescript/greeting.ts']);
    expect(map.files['.hl/src/typescript/greeting.ts']).toBeUndefined();
    expect(map.files['.hl/src/typescript/greeting.test.ts']).toBeUndefined();
  });

  it('is cleared by a subsequent successful recordAttribution', () => {
    const map = seeded();
    recordUnattributed(map, {
      rel: 'examples/greeting.hl',
      module: 'greeting',
      promptHash: 'p1',
      target: 'typescript',
      declaredTargets: ['typescript'],
      files: ['.hl/src/typescript/greeting.ts'],
    });
    recordAttribution(map, {
      rel: 'examples/greeting.hl',
      module: 'greeting',
      promptHash: 'p2',
      target: 'typescript',
      declaredTargets: ['typescript'],
      files: [{ filePath: '.hl/src/typescript/greeting.ts', hash: 'h9' }],
    });
    const entry = map.prompts['examples/greeting.hl'].targets.typescript;
    expect(entry.unattributed).toBeUndefined();
    expect(entry.promptHashAtGen).toBe('p2');
  });
});
