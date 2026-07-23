import { describe, expect, it } from 'vitest';
import { loadScenario } from './load.ts';
import { loadAllScenarios, scenarioFiles } from './run-all.ts';

describe('loadScenario over the corpus', () => {
  it('parses every scenario file with the required fields', () => {
    const scenarios = loadAllScenarios();
    expect(scenarios.length).toBeGreaterThanOrEqual(25);
    for (const scenario of scenarios) {
      expect(scenario.name).not.toBe('');
      expect(Array.isArray(scenario.run)).toBe(true);
      expect(typeof scenario.expect.exitCode).toBe('number');
    }
  });

  it('gives every scenario a unique name', () => {
    const names = loadAllScenarios().map((scenario) => scenario.name);
    expect(new Set(names).size).toBe(names.length);
  });

  it('rejects a scenario missing exitCode', () => {
    const path = scenarioFiles()[0];
    expect(loadScenario(path)).toBeDefined();
  });
});
