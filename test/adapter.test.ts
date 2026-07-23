import { describe, expect, it } from 'vitest';
import { getAdapter, listTargets } from '../src/targets/registry.js';

describe('target adapter interface', () => {
  it('exposes the typescript adapter as an agentic config object', () => {
    const adapter = getAdapter('typescript');
    expect(adapter.name).toBe('typescript');
    expect(typeof adapter.idiomGuidance).toBe('string');
    expect(adapter.idiomGuidance.length).toBeGreaterThan(0);
    expect(adapter.agentTools).toContain('Write');
    expect(adapter.attributionExcludeDirs).toContain('node_modules');
    expect(adapter.attributionExcludeFiles).toContain('package-lock.json');
  });

  it('exposes the react adapter with jsdom + testing-library guidance', () => {
    const adapter = getAdapter('react');
    expect(adapter.name).toBe('react');
    expect(adapter.idiomGuidance).toMatch(/vite/i);
    expect(adapter.idiomGuidance).toMatch(/jsdom/i);
    expect(adapter.idiomGuidance).toMatch(/testing-library/i);
    expect(adapter.attributionExcludeFiles).toContain('package-lock.json');
  });

  it('builds a test-run command that runs inside the target directory', () => {
    const cmd = getAdapter('react').testCommand('/repo/.hl/src/react');
    expect(cmd.command).toBe('npx');
    expect(cmd.args).toContain('vitest');
    expect(cmd.args).toContain('run');
  });

  it('lists available targets and rejects unknown ones', () => {
    expect(listTargets()).toContain('typescript');
    expect(listTargets()).toContain('react');
    expect(() => getAdapter('rust')).toThrow(/unknown target/);
  });
});
