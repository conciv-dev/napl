import { describe, expect, it } from 'vitest';
import type { AttributionEntry } from '../src/core/attribution-schema.js';
import {
  diffBodyLines,
  incrementalUnlockList,
  selectIntersectingEntries,
} from '../src/core/incremental.js';

function entry(promptLines: [number, number], file: string, lines: [number, number], note = ''): AttributionEntry {
  return { promptLines, file, lines, note };
}

describe('diffBodyLines', () => {
  it('returns empty unified diff and no changed lines for identical bodies', () => {
    const body = 'line one\nline two\nline three';
    const diff = diffBodyLines(body, body);
    expect(diff.unified).toBe('');
    expect(diff.changedOldLines).toEqual([]);
    expect(diff.changedNewLines).toEqual([]);
  });

  it('reports a single changed line as one del and one ins with a hunk header', () => {
    const oldBody = 'a\nThe greeting ends with an exclamation mark.\nc';
    const newBody = 'a\nThe greeting ends with a period.\nc';
    const diff = diffBodyLines(oldBody, newBody);
    expect(diff.changedOldLines).toEqual([2]);
    expect(diff.changedNewLines).toEqual([2]);
    expect(diff.unified).toContain('@@');
    expect(diff.unified).toContain('-The greeting ends with an exclamation mark.');
    expect(diff.unified).toContain('+The greeting ends with a period.');
    expect(diff.unified).toContain(' a');
    expect(diff.unified).toContain(' c');
  });

  it('reports added lines as changedNewLines only', () => {
    const oldBody = 'a\nb';
    const newBody = 'a\nb\nc';
    const diff = diffBodyLines(oldBody, newBody);
    expect(diff.changedOldLines).toEqual([]);
    expect(diff.changedNewLines).toEqual([3]);
  });

  it('reports removed lines as changedOldLines only', () => {
    const oldBody = 'a\nb\nc';
    const newBody = 'a\nc';
    const diff = diffBodyLines(oldBody, newBody);
    expect(diff.changedOldLines).toEqual([2]);
    expect(diff.changedNewLines).toEqual([]);
  });
});

describe('selectIntersectingEntries', () => {
  const entries: AttributionEntry[] = [
    entry([1, 2], 'src/a.ts', [1, 5], 'signature'),
    entry([6, 6], 'src/a.ts', [10, 12], 'greeting format'),
    entry([7, 8], 'src/a.ts', [14, 20], 'trimming'),
  ];

  it('selects only entries whose promptLines overlap a changed old line', () => {
    const selected = selectIntersectingEntries(entries, [6]);
    expect(selected).toHaveLength(1);
    expect(selected[0].note).toBe('greeting format');
  });

  it('selects multiple entries when the change spans several prompt lines', () => {
    const selected = selectIntersectingEntries(entries, [2, 7]);
    expect(selected.map((entry) => entry.note)).toEqual(['signature', 'trimming']);
  });

  it('returns nothing when no entry covers the changed lines', () => {
    expect(selectIntersectingEntries(entries, [99])).toEqual([]);
  });
});

describe('incrementalUnlockList', () => {
  it('unions owned files with the target-resolved intersecting entry files, sorted and deduped', () => {
    const owned = ['.hl/src/typescript/src/greeting.ts', '.hl/src/typescript/package.json'];
    const entries = [entry([6, 6], 'src/greeting.ts', [1, 1]), entry([7, 7], 'src/new.ts', [1, 1])];
    const list = incrementalUnlockList(owned, entries, '.hl/src/typescript');
    expect(list).toEqual([
      '.hl/src/typescript/package.json',
      '.hl/src/typescript/src/greeting.ts',
      '.hl/src/typescript/src/new.ts',
    ]);
  });
});
