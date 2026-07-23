import { describe, expect, it } from 'vitest';
import { applyTemplate, normalizePaths } from './template.ts';

describe('applyTemplate', () => {
  it('substitutes {{CWD}} and {{RUNNER_PID}}', () => {
    const out = applyTemplate('root={{CWD}} pid={{RUNNER_PID}}', { cwd: '/work', runnerPid: 42 });
    expect(out).toBe('root=/work pid=42');
  });

  it('substitutes every occurrence', () => {
    const out = applyTemplate('{{CWD}}/a {{CWD}}/b', { cwd: '/w', runnerPid: 1 });
    expect(out).toBe('/w/a /w/b');
  });
});

describe('normalizePaths', () => {
  it('replaces the workdir path with {{CWD}}', () => {
    expect(normalizePaths('at /var/x/greet.ts', '/var/x', '/var/x')).toBe('at {{CWD}}/greet.ts');
  });

  it('replaces the realpath variant as well', () => {
    const out = normalizePaths('at /private/var/x/greet.ts', '/var/x', '/private/var/x');
    expect(out).toBe('at {{CWD}}/greet.ts');
  });
});
