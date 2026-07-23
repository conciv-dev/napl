import { describe, expect, it } from 'vitest';
import { DEFAULT_BACKEND, parseLock } from '../src/core/lock.js';

describe('parseLock', () => {
  it('defaults backend to claude-cli when the field is missing', () => {
    const lock = parseLock(JSON.stringify({ model: 'claude-sonnet-5' }));
    expect(lock.backend).toBe('claude-cli');
    expect(DEFAULT_BACKEND).toBe('claude-cli');
  });

  it('keeps an explicit anthropic-api backend', () => {
    const lock = parseLock(JSON.stringify({ model: 'claude-sonnet-5', backend: 'anthropic-api' }));
    expect(lock.backend).toBe('anthropic-api');
  });

  it('keeps an explicit claude-cli backend', () => {
    const lock = parseLock(JSON.stringify({ model: 'claude-opus-5', backend: 'claude-cli' }));
    expect(lock.backend).toBe('claude-cli');
    expect(lock.model).toBe('claude-opus-5');
  });

  it('rejects an unknown backend value', () => {
    expect(() => parseLock(JSON.stringify({ model: 'x', backend: 'openai' }))).toThrow(/invalid lock\.json/);
  });

  it('rejects corrupt json', () => {
    expect(() => parseLock('{not json')).toThrow(/corrupt lock\.json/);
  });
});
