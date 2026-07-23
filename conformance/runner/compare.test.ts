import { describe, expect, it } from 'vitest';
import { compareScenario } from './compare.ts';
import type { ActualResult } from './execute.ts';
import type { Scenario } from './types.ts';

function actual(overrides: Partial<ActualResult>): ActualResult {
  return {
    exitCode: 0,
    stdout: '',
    stderr: '',
    files: new Map(),
    agentInputs: [],
    workdir: '/tmp/work',
    ...overrides,
  };
}

function scenario(expectPart: Scenario['expect']): Scenario {
  return { name: 'x', description: 'd', run: ['status'], expect: expectPart };
}

describe('compareScenario exit code', () => {
  it('passes when the exit code matches', () => {
    const result = compareScenario(scenario({ exitCode: 0 }), actual({ exitCode: 0 }));
    expect(result).toEqual([]);
  });

  it('fails when the exit code differs', () => {
    const result = compareScenario(scenario({ exitCode: 1 }), actual({ exitCode: 0 }));
    expect(result).toHaveLength(1);
    expect(result[0].kind).toBe('exitCode');
  });
});

describe('compareScenario stdout lines', () => {
  it('matches exact and regex-marked lines', () => {
    const result = compareScenario(
      scenario({ exitCode: 0, stdout: ['clean       a', 're:^gen #\\d+'] }),
      actual({ stdout: 'clean       a\ngen #7 more\n' }),
    );
    expect(result).toEqual([]);
  });

  it('reports a line-count mismatch', () => {
    const result = compareScenario(
      scenario({ exitCode: 0, stdout: ['only one'] }),
      actual({ stdout: 'first\nsecond\n' }),
    );
    expect(result[0].kind).toBe('stdout line count');
  });

  it('reports a differing line', () => {
    const result = compareScenario(
      scenario({ exitCode: 0, stdout: ['expected'] }),
      actual({ stdout: 'actual\n' }),
    );
    expect(result[0].kind).toBe('stdout line 1');
  });
});

describe('compareScenario contains', () => {
  it('passes when every substring is present', () => {
    const result = compareScenario(
      scenario({ exitCode: 0, stdoutContains: ['generated 1', 'skipped 0'] }),
      actual({ stdout: 'generated 1, skipped 0\n' }),
    );
    expect(result).toEqual([]);
  });

  it('fails on a missing substring', () => {
    const result = compareScenario(
      scenario({ exitCode: 0, stdoutContains: ['nope'] }),
      actual({ stdout: 'something else\n' }),
    );
    expect(result[0].kind).toBe('stdout contains');
  });
});

describe('compareScenario files', () => {
  it('matches exact content, mode, and absence', () => {
    const files = new Map([
      ['a.txt', { exists: true, content: 'hi\n', mode: '0444' }],
      ['gone.txt', { exists: false, content: '', mode: '' }],
    ]);
    const result = compareScenario(
      scenario({
        exitCode: 0,
        files: { 'a.txt': { content: 'hi\n', mode: '0444' }, 'gone.txt': { absent: true } },
      }),
      actual({ files }),
    );
    expect(result).toEqual([]);
  });

  it('flags a content mismatch and a wrong mode', () => {
    const files = new Map([['a.txt', { exists: true, content: 'no\n', mode: '0644' }]]);
    const result = compareScenario(
      scenario({ exitCode: 0, files: { 'a.txt': { content: 'yes\n', mode: '0444' } } }),
      actual({ files }),
    );
    expect(result.map((failure) => failure.kind).sort()).toEqual(['file content', 'file mode']);
  });

  it('flags a file that should be absent but is present', () => {
    const files = new Map([['a.txt', { exists: true, content: 'x', mode: '0644' }]]);
    const result = compareScenario(
      scenario({ exitCode: 0, files: { 'a.txt': { absent: true } } }),
      actual({ files }),
    );
    expect(result[0].detail).toContain('expected absent but present');
  });
});

describe('compareScenario agent inputs and RUNNER_PID', () => {
  it('substitutes {{RUNNER_PID}} in expected stderr', () => {
    const result = compareScenario(
      scenario({ exitCode: 1, stderr: ['held by pid {{RUNNER_PID}}'] }),
      actual({ exitCode: 1, stderr: `held by pid ${process.pid}\n` }),
    );
    expect(result).toEqual([]);
  });

  it('checks captured agent input substrings', () => {
    const result = compareScenario(
      scenario({ exitCode: 0, agentInputs: [{ index: 0, contains: ['INCREMENTAL'] }] }),
      actual({ agentInputs: ['an INCREMENTAL update task'] }),
    );
    expect(result).toEqual([]);
  });

  it('fails when the agent input at an index is missing', () => {
    const result = compareScenario(
      scenario({ exitCode: 0, agentInputs: [{ index: 1, contains: ['x'] }] }),
      actual({ agentInputs: ['only one'] }),
    );
    expect(result[0].kind).toBe('agentInput');
  });
});
