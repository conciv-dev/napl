import { describe, expect, it } from 'vitest';
import { extractCodeFiles, extractYaml } from '../src/core/parse-output.js';

describe('extractYaml', () => {
  it('extracts a fenced yaml block', () => {
    const out = extractYaml('prose\n```yaml\nmodule: x\n```\ntrailing');
    expect(out).toBe('module: x');
  });

  it('falls back to trimmed text without a fence', () => {
    expect(extractYaml('  module: x  ')).toBe('module: x');
  });
});

describe('extractCodeFiles', () => {
  it('parses multiple labelled file blocks', () => {
    const text = [
      '=== FILE: greeting.ts ===',
      '```typescript',
      'export const greet = (n: string) => `Hello, ${n}!`;',
      '```',
      '=== FILE: greeting.test.ts ===',
      '```typescript',
      'import { greet } from "./greeting";',
      '```',
    ].join('\n');
    const files = extractCodeFiles(text);
    expect(files).toHaveLength(2);
    expect(files[0].path).toBe('greeting.ts');
    expect(files[0].content).toContain('export const greet');
    expect(files[1].path).toBe('greeting.test.ts');
  });

  it('returns an empty array when no blocks are present', () => {
    expect(extractCodeFiles('just prose')).toEqual([]);
  });
});
