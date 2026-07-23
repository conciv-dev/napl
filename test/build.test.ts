import { describe, expect, it, vi } from 'vitest';
import { runBuild } from '../src/commands/build.js';

describe('runBuild (deprecated)', () => {
  it('prints a deprecation notice pointing to hl gen and does nothing else', async () => {
    const log = vi.fn();
    await runBuild({ log });
    expect(log).toHaveBeenCalledTimes(1);
    expect(log.mock.calls[0][0]).toMatch(/deprecated/i);
    expect(log.mock.calls[0][0]).toMatch(/hl gen/);
  });
});
