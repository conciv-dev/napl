import { mkdtemp, mkdir, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { detectGenDrift, formatGenDriftReport } from '../src/core/drift.js';
import { contentHash } from '../src/core/hash.js';
import { filePatch } from '../src/core/journal.js';
import type { JournalEntry } from '../src/core/journal.js';
import { emptyMap, recordAttribution } from '../src/core/map.js';
import type { NaplMap } from '../src/core/map.js';

let root: string;

const BASELINE = 'export const greet = (n: string) => `Hello, ${n}!`;\n';
const EDITED = 'export const greet = (n: string) => `HELLO, ${n}!`;\n';
const FILE = '.napl/src/typescript/greet.ts';

async function writeSrc(content: string): Promise<void> {
  await mkdir(join(root, '.napl', 'src', 'typescript'), { recursive: true });
  await writeFile(join(root, FILE), content, 'utf8');
}

function mapWith(hash: string): NaplMap {
  const map = emptyMap();
  recordAttribution(map, {
    rel: 'examples/greeting.napl',
    module: 'greeting',
    promptHash: 'ph',
    target: 'typescript',
    declaredTargets: ['typescript'],
    files: [{ filePath: FILE, hash }],
  });
  return map;
}

function journalWith(): JournalEntry[] {
  return [
    {
      gen: 1,
      timestamp: 't1',
      module: 'greeting',
      target: 'typescript',
      promptHash: 'ph',
      promptDiff: '',
      mode: 'full',
      files: [{ path: FILE, patch: filePatch(null, BASELINE), hashBefore: null, hashAfter: contentHash(BASELINE) }],
    },
  ];
}

beforeEach(async () => {
  root = await mkdtemp(join(tmpdir(), 'napl-drift-'));
});

afterEach(async () => {
  await rm(root, { recursive: true, force: true });
});

describe('detectGenDrift', () => {
  it('reports no drift when the file matches its recorded hash', async () => {
    await writeSrc(BASELINE);
    const drifts = await detectGenDrift({ root, target: 'typescript', map: mapWith(contentHash(BASELINE)), journal: [] });
    expect(drifts).toEqual([]);
  });

  it('reports drift with a reconstructed baseline diff when the file was edited', async () => {
    await writeSrc(EDITED);
    const drifts = await detectGenDrift({
      root,
      target: 'typescript',
      map: mapWith(contentHash(BASELINE)),
      journal: journalWith(),
    });
    expect(drifts).toHaveLength(1);
    expect(drifts[0].module).toBe('greeting');
    expect(drifts[0].files[0].reason).toBe('edited');
    expect(drifts[0].files[0].baseline).toBe(BASELINE);
    expect(drifts[0].files[0].diff).toContain('-export const greet = (n: string) => `Hello');
    expect(drifts[0].files[0].diff).toContain('+export const greet = (n: string) => `HELLO');
  });

  it('falls back to hashes only when the journal cannot reconstruct the baseline', async () => {
    await writeSrc(EDITED);
    const drifts = await detectGenDrift({ root, target: 'typescript', map: mapWith(contentHash(BASELINE)), journal: [] });
    expect(drifts[0].files[0].baseline).toBeNull();
    expect(drifts[0].files[0].diff).toBeNull();
    expect(drifts[0].files[0].expectedHash).toBe(contentHash(BASELINE));
    expect(drifts[0].files[0].actualHash).toBe(contentHash(EDITED));
  });

  it('reports a missing locked file as drift', async () => {
    const drifts = await detectGenDrift({ root, target: 'typescript', map: mapWith(contentHash(BASELINE)), journal: [] });
    expect(drifts[0].files[0].reason).toBe('missing');
  });

  it('honours the module scope, ignoring drift in other modules', async () => {
    await writeSrc(EDITED);
    const drifts = await detectGenDrift({
      root,
      target: 'typescript',
      map: mapWith(contentHash(BASELINE)),
      journal: journalWith(),
      moduleScope: 'somethingElse',
    });
    expect(drifts).toEqual([]);
  });

  it('ignores an unattributed target record', async () => {
    await writeSrc(EDITED);
    const map = mapWith(contentHash(BASELINE));
    map.prompts['examples/greeting.napl'].targets.typescript.unattributed = true;
    const drifts = await detectGenDrift({ root, target: 'typescript', map, journal: journalWith() });
    expect(drifts).toEqual([]);
  });
});

describe('formatGenDriftReport', () => {
  it('renders the three verbatim resolution commands', () => {
    const report = formatGenDriftReport(
      [{ module: 'greeting', promptFile: 'examples/greeting.napl', target: 'typescript', files: [{ file: FILE, reason: 'edited', baseline: BASELINE, current: EDITED, diff: '@@ x @@' }] }],
      'typescript',
    );
    expect(report).toContain('napl reconcile greeting');
    expect(report).toContain('(coming soon)');
    expect(report).toContain('napl gen typescript --module greeting --force');
    expect(report).toContain('edit the prompt to describe the change, then napl gen typescript');
    expect(report).toContain("cannot run 'napl gen typescript'");
  });
});
