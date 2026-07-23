import { describe, expect, it } from 'vitest';
import { parse as parseYaml } from 'yaml';
import {
  entriesAtBodyLine,
  parseAttributionEntries,
  validateAttribution,
} from '../src/core/attribution-schema.js';
import type { Attribution } from '../src/core/attribution-schema.js';
import { bodyLineForDocLine, numberLines, promptBodyLines } from '../src/core/body-lines.js';
import { extractYaml } from '../src/core/parse-output.js';

const ATTRIBUTION: Attribution = {
  module: 'greeting',
  target: 'typescript',
  entries: [
    { promptLines: [2, 2], file: 'greeting.ts', lines: [1, 3], note: 'builds the greeting' },
    { promptLines: [3, 4], file: 'greeting.ts', lines: [5, 7], note: 'trims whitespace' },
    { promptLines: [3, 3], file: 'greeting.test.ts', lines: [10, 12], note: 'covers trimming' },
  ],
};

describe('attribution schema', () => {
  it('validates a well-formed attribution document', () => {
    const parsed = validateAttribution(ATTRIBUTION);
    expect(parsed.entries).toHaveLength(3);
  });

  it('rejects an entry with a missing file', () => {
    expect(() =>
      validateAttribution({
        module: 'm',
        target: 't',
        entries: [{ promptLines: [1, 1], lines: [1, 1], note: 'x' }],
      }),
    ).toThrow(/attribution validation failed/);
  });

  it('rejects a non-integer line range', () => {
    expect(() =>
      validateAttribution({
        module: 'm',
        target: 't',
        entries: [{ promptLines: [1, 1], file: 'a.ts', lines: [1.5, 2], note: 'x' }],
      }),
    ).toThrow(/attribution validation failed/);
  });
});

describe('parseAttributionEntries (mocked LLM output)', () => {
  it('parses a fenced yaml list produced by the model', () => {
    const modelResponse = [
      'Here is the mapping:',
      '```yaml',
      '- promptLines: [2, 2]',
      '  file: greeting.ts',
      '  lines: [1, 3]',
      '  note: builds the greeting',
      '- promptLines: [3, 4]',
      '  file: greeting.ts',
      '  lines: [5, 7]',
      '  note: trims whitespace',
      '```',
    ].join('\n');
    const entries = parseAttributionEntries(parseYaml(extractYaml(modelResponse)));
    expect(entries).toHaveLength(2);
    expect(entries[1].note).toBe('trims whitespace');
    expect(entries[1].lines).toEqual([5, 7]);
  });

  it('normalizes a single-line range emitted as [n] or n', () => {
    const entries = parseAttributionEntries([
      { promptLines: [8], file: 'a.ts', lines: 3, note: 'single line' },
    ]);
    expect(entries[0].promptLines).toEqual([8, 8]);
    expect(entries[0].lines).toEqual([3, 3]);
  });

  it('throws on a malformed list', () => {
    expect(() => parseAttributionEntries([{ promptLines: 'nope', file: 'a', lines: [1, 2] }])).toThrow(
      /attribution entries invalid/,
    );
  });
});

describe('entriesAtBodyLine (range lookup)', () => {
  it('returns every entry whose prompt range contains the line', () => {
    const atThree = entriesAtBodyLine(ATTRIBUTION, 3);
    expect(atThree.map((e) => e.note).sort()).toEqual(['covers trimming', 'trims whitespace']);
  });

  it('returns a single entry for a line in one range only', () => {
    expect(entriesAtBodyLine(ATTRIBUTION, 2).map((e) => e.note)).toEqual(['builds the greeting']);
  });

  it('returns nothing when no range covers the line', () => {
    expect(entriesAtBodyLine(ATTRIBUTION, 9)).toEqual([]);
  });
});

describe('body line mapping', () => {
  const RAW = ['---', 'module: greeting', '---', 'First body line.', 'Second body line.'].join('\n');

  it('locates the body start after the frontmatter', () => {
    const body = promptBodyLines(RAW);
    expect(body.bodyStartLine).toBe(3);
    expect(body.lines[0]).toBe('First body line.');
  });

  it('maps document lines to 1-based body lines', () => {
    const body = promptBodyLines(RAW);
    expect(bodyLineForDocLine(body, 3)).toBe(1);
    expect(bodyLineForDocLine(body, 4)).toBe(2);
    expect(bodyLineForDocLine(body, 2)).toBeNull();
    expect(bodyLineForDocLine(body, 99)).toBeNull();
  });

  it('numbers lines 1-based for the model prompt', () => {
    expect(numberLines(['a', 'b'])).toBe('1: a\n2: b');
  });
});
