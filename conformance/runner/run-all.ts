import { readdirSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { compareScenario } from './compare.ts';
import { executeScenario } from './execute.ts';
import { loadScenario } from './load.ts';
import type { Scenario, ScenarioResult } from './types.ts';

const HERE = dirname(fileURLToPath(import.meta.url));
export const SCENARIOS_DIR = join(HERE, '..', 'scenarios');

export function scenarioFiles(): string[] {
  return readdirSync(SCENARIOS_DIR)
    .filter((name) => name.endsWith('.yaml'))
    .sort()
    .map((name) => join(SCENARIOS_DIR, name));
}

export function loadAllScenarios(): Scenario[] {
  return scenarioFiles().map((path) => loadScenario(path));
}

export function runScenario(scenario: Scenario): ScenarioResult {
  const started = Date.now();
  const actual = executeScenario(scenario);
  const failures = compareScenario(scenario, actual);
  return {
    name: scenario.name,
    description: scenario.description,
    passed: failures.length === 0,
    failures,
    durationMs: Date.now() - started,
  };
}
