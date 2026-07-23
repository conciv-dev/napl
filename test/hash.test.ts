import { describe, expect, it } from 'vitest';
import { contentHash } from '../src/core/hash.js';

describe('contentHash', () => {
  it('is deterministic for identical content', () => {
    expect(contentHash('hello')).toBe(contentHash('hello'));
  });

  it('differs for different content', () => {
    expect(contentHash('hello')).not.toBe(contentHash('world'));
  });

  it('produces a 64-char hex sha256', () => {
    expect(contentHash('x')).toMatch(/^[0-9a-f]{64}$/);
  });
});
