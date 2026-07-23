import { describe, expect, it } from 'vitest';
import {
  codeLensTitle,
  dedupeMatches,
  DRIFT_LENS_PREFIX,
  isFileDrifted,
  parseGeneratedPath,
  promptAbsoluteLines,
  reverseMatches,
} from '../src/lsp/reverse.js';
import type { AttributionSource } from '../src/lsp/reverse.js';

const SOURCES: AttributionSource[] = [
  {
    module: 'greeting',
    target: 'typescript',
    promptFiles: ['examples/greeting.hl'],
    entries: [
      { promptLines: [3, 4], file: 'src/greeting.ts', lines: [1, 1], note: 'greet function signature' },
      { promptLines: [7, 7], file: 'src/greeting.ts', lines: [2, 2], note: 'trims name whitespace' },
      { promptLines: [8, 8], file: 'src/greeting.ts', lines: [3, 5], note: 'rejects empty name' },
      { promptLines: [6, 6], file: 'src/greeting.ts', lines: [6, 6], note: 'builds greeting message' },
      { promptLines: [7, 7], file: 'src/greeting.test.ts', lines: [9, 11], note: 'test trimming' },
    ],
  },
];

describe('parseGeneratedPath', () => {
  it('splits a generated path into target and repo-relative path', () => {
    expect(parseGeneratedPath('.hl/src/typescript/src/greeting.ts')).toEqual({
      target: 'typescript',
      targetRelPath: 'src/greeting.ts',
    });
  });

  it('normalizes windows separators', () => {
    expect(parseGeneratedPath('.hl\\src\\typescript\\src\\greeting.ts')).toEqual({
      target: 'typescript',
      targetRelPath: 'src/greeting.ts',
    });
  });

  it('returns null for non-generated paths', () => {
    expect(parseGeneratedPath('examples/greeting.hl')).toBeNull();
    expect(parseGeneratedPath('.hl/src/typescript')).toBeNull();
    expect(parseGeneratedPath('src/greeting.ts')).toBeNull();
  });
});

describe('reverseMatches', () => {
  it('finds the single entry whose code range contains the line', () => {
    const matches = reverseMatches(SOURCES, 'typescript', 'src/greeting.ts', 2);
    expect(matches.map((m) => m.note)).toEqual(['trims name whitespace']);
    expect(matches[0].promptLines).toEqual([7, 7]);
    expect(matches[0].promptFile).toBe('examples/greeting.hl');
  });

  it('matches a multi-line code range', () => {
    const matches = reverseMatches(SOURCES, 'typescript', 'src/greeting.ts', 4);
    expect(matches.map((m) => m.note)).toEqual(['rejects empty name']);
  });

  it('returns nothing when target differs', () => {
    expect(reverseMatches(SOURCES, 'swift', 'src/greeting.ts', 2)).toEqual([]);
  });

  it('returns nothing when the file does not match', () => {
    expect(reverseMatches(SOURCES, 'typescript', 'src/other.ts', 2)).toEqual([]);
  });

  it('returns all entries for the file when codeLine is null', () => {
    const matches = reverseMatches(SOURCES, 'typescript', 'src/greeting.ts', null);
    expect(matches).toHaveLength(4);
  });

  it('merges multiple contributing prompts into separate matches', () => {
    const multi: AttributionSource[] = [
      {
        module: 'greeting',
        target: 'typescript',
        promptFiles: ['examples/a.hl', 'examples/b.hl'],
        entries: [{ promptLines: [1, 1], file: 'src/x.ts', lines: [4, 4], note: 'shared' }],
      },
    ];
    const matches = reverseMatches(multi, 'typescript', 'src/x.ts', 4);
    expect(matches.map((m) => m.promptFile)).toEqual(['examples/a.hl', 'examples/b.hl']);
  });
});

describe('promptAbsoluteLines', () => {
  it('converts body-relative 1-based prompt lines to absolute 0-based doc lines', () => {
    expect(promptAbsoluteLines(12, [7, 7])).toEqual([18, 18]);
    expect(promptAbsoluteLines(12, [3, 4])).toEqual([14, 15]);
  });

  it('matches the forward-direction offset formula', () => {
    const bodyStart = 3;
    expect(promptAbsoluteLines(bodyStart, [1, 1])).toEqual([3, 3]);
    expect(promptAbsoluteLines(bodyStart, [2, 2])).toEqual([4, 4]);
  });
});

describe('isFileDrifted', () => {
  it('reports drift when hashes differ', () => {
    expect(isFileDrifted('aaa', 'bbb')).toBe(true);
  });

  it('reports no drift when hashes match', () => {
    expect(isFileDrifted('aaa', 'aaa')).toBe(false);
  });

  it('reports no drift when no hash was recorded', () => {
    expect(isFileDrifted(undefined, 'bbb')).toBe(false);
  });
});

describe('codeLensTitle', () => {
  it('formats a lens title with a note', () => {
    expect(codeLensTitle('greeting.hl', 19, 'trims name whitespace')).toBe(
      '⇠ greeting.hl:19 — trims name whitespace',
    );
  });

  it('omits the dash when the note is empty', () => {
    expect(codeLensTitle('greeting.hl', 19, '')).toBe('⇠ greeting.hl:19');
  });

  it('composes with the DRIFT prefix', () => {
    const base = codeLensTitle('greeting.hl', 19, 'trims');
    expect(`${DRIFT_LENS_PREFIX}   ${base}`).toContain('DRIFT — edits here are not reflected');
  });
});

describe('dedupeMatches', () => {
  it('collapses duplicate prompt spans', () => {
    const matches = reverseMatches(SOURCES, 'typescript', 'src/greeting.ts', null);
    const withDupes = [...matches, ...matches];
    expect(dedupeMatches(withDupes)).toHaveLength(matches.length);
  });
});
