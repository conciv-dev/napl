import { describe, expect, it } from 'vitest';
import { compareScenario } from './compare.ts';
import { executeScenario } from './execute.ts';
import { loadScenario } from './load.ts';
import { scenarioFiles } from './run-all.ts';

function scenarioByName(name: string) {
  for (const path of scenarioFiles()) {
    const scenario = loadScenario(path);
    if (scenario.name === name) return scenario;
  }
  throw new Error(`scenario ${name} not found`);
}

describe('harness end-to-end (requires built CLI)', () => {
  it('runs the version scenario against the real CLI and passes', () => {
    const scenario = scenarioByName('version');
    const actual = executeScenario(scenario);
    expect(compareScenario(scenario, actual)).toEqual([]);
  });

  it('detects a mismatch when the expectation is wrong', () => {
    const scenario = scenarioByName('version');
    const broken = { ...scenario, expect: { ...scenario.expect, exitCode: 99 } };
    const actual = executeScenario(broken);
    const failures = compareScenario(broken, actual);
    expect(failures.some((failure) => failure.kind === 'exitCode')).toBe(true);
  });
});
