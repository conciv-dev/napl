import { mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import {
  appendJournalEntry,
  fileHistory,
  filePatch,
  nextGenNumber,
  readJournal,
  reconstructFileContent,
} from '../src/core/journal.js';
import type { JournalEntry } from '../src/core/journal.js';

let dir: string;

function entry(gen: number, path: string): JournalEntry {
  return {
    gen,
    timestamp: `t${gen}`,
    module: 'demo',
    target: 'react',
    promptHash: `h${gen}`,
    promptDiff: '',
    mode: 'full',
    files: [{ path, patch: filePatch(null, 'x\n'), hashBefore: null, hashAfter: 'abc' }],
  };
}

beforeEach(async () => {
  dir = await mkdtemp(join(tmpdir(), 'napl-journal-'));
});

afterEach(async () => {
  await rm(dir, { recursive: true, force: true });
});

describe('reconstructFileContent', () => {
  function withPatch(gen: number, path: string, before: string | null, after: string): JournalEntry {
    return {
      gen,
      timestamp: `t${gen}`,
      module: 'demo',
      target: 'react',
      promptHash: `h${gen}`,
      promptDiff: '',
      mode: 'full',
      files: [{ path, patch: filePatch(before, after), hashBefore: null, hashAfter: 'x' }],
    };
  }

  it('replays patches oldest-to-newest to rebuild the last recorded content', () => {
    const entries = [
      withPatch(1, 'a.ts', null, 'line one\nline two\n'),
      withPatch(2, 'a.ts', 'line one\nline two\n', 'line one\nline TWO\nline three\n'),
    ];
    expect(reconstructFileContent(entries, 'a.ts')).toBe('line one\nline TWO\nline three\n');
  });

  it('returns null when the file never appears in the journal', () => {
    expect(reconstructFileContent([withPatch(1, 'a.ts', null, 'x\n')], 'b.ts')).toBeNull();
  });
});

describe('filePatch', () => {
  it('produces an all-insert unified diff for a created file', () => {
    const patch = filePatch(null, 'a\nb\n');
    expect(patch).toContain('@@ -0,0 +1,2 @@');
    expect(patch).toContain('+a');
    expect(patch).toContain('+b');
  });

  it('produces a scoped diff for a modified file', () => {
    const patch = filePatch('a\nb\nc\n', 'a\nB\nc\n');
    expect(patch).toContain('-b');
    expect(patch).toContain('+B');
  });
});

describe('readJournal / appendJournalEntry', () => {
  it('round-trips appended entries', async () => {
    const path = join(dir, 'journal.jsonl');
    await appendJournalEntry(path, entry(1, 'f.ts'));
    await appendJournalEntry(path, entry(2, 'f.ts'));
    const entries = await readJournal(path);
    expect(entries.map((e) => e.gen)).toEqual([1, 2]);
  });

  it('returns an empty list when the journal does not exist', async () => {
    expect(await readJournal(join(dir, 'missing.jsonl'))).toEqual([]);
  });

  it('skips corrupt (unparseable) and schema-invalid lines with a warning, keeping valid ones', async () => {
    const path = join(dir, 'journal.jsonl');
    const valid = JSON.stringify(entry(1, 'f.ts'));
    const invalidSchema = JSON.stringify({ gen: 'nope', module: 'x' });
    await writeFile(path, `${valid}\nnot json at all\n${invalidSchema}\n`, 'utf8');
    const warnings: string[] = [];
    const entries = await readJournal(path, (m) => warnings.push(m));
    expect(entries.map((e) => e.gen)).toEqual([1]);
    expect(warnings).toHaveLength(2);
  });
});

describe('nextGenNumber', () => {
  it('is 1 for an empty journal', () => {
    expect(nextGenNumber([])).toBe(1);
  });

  it('is one past the highest recorded gen', () => {
    expect(nextGenNumber([entry(3, 'a'), entry(7, 'b'), entry(5, 'c')])).toBe(8);
  });
});

describe('fileHistory', () => {
  it('returns only the entries touching the given file, carrying its patch', () => {
    const entries: JournalEntry[] = [entry(1, 'a.ts'), entry(2, 'b.ts'), entry(3, 'a.ts')];
    const history = fileHistory(entries, 'a.ts');
    expect(history.map((h) => h.gen)).toEqual([1, 3]);
    expect(history[0].patch).toContain('+x');
  });

  it('is empty for a file with no history', () => {
    expect(fileHistory([entry(1, 'a.ts')], 'missing.ts')).toEqual([]);
  });
});
