import { existsSync } from 'node:fs';
import { executeScenario } from './execute.ts';
import { loadScenario } from './load.ts';
import { formatResult, formatSummary } from './report.ts';
import { loadAllScenarios, runScenario, scenarioFiles } from './run-all.ts';
import type { ScenarioResult } from './types.ts';

function findScenario(name: string) {
  for (const path of scenarioFiles()) {
    const scenario = loadScenario(path);
    if (scenario.name === name) return scenario;
  }
  throw new Error(`no scenario named "${name}"`);
}

function dump(name: string): void {
  const scenario = findScenario(name);
  const actual = executeScenario(scenario, { collectAll: true });
  process.stdout.write(`exitCode: ${actual.exitCode}\n`);
  process.stdout.write(`\n=== stdout ===\n${actual.stdout}`);
  process.stdout.write(`\n=== stderr ===\n${actual.stderr}`);
  process.stdout.write(`\n=== agent inputs (${actual.agentInputs.length}) ===\n`);
  actual.agentInputs.forEach((input, index) => {
    process.stdout.write(`--- agent-input-${index + 1} ---\n${input}\n`);
  });
  process.stdout.write('\n=== files ===\n');
  const files = [...(actual.allFiles ?? new Map())].sort(([a], [b]) => (a < b ? -1 : 1));
  for (const [rel, file] of files) {
    process.stdout.write(`\n----- ${rel} (mode ${file.mode}) -----\n${file.content}`);
  }
}

function run(filter: string | undefined): void {
  const scenarios = loadAllScenarios().filter((scenario) => filter === undefined || scenario.name === filter);
  if (scenarios.length === 0) {
    process.stderr.write(filter === undefined ? 'no scenarios found\n' : `no scenario named "${filter}"\n`);
    process.exitCode = 1;
    return;
  }
  const started = Date.now();
  const results: ScenarioResult[] = [];
  for (const scenario of scenarios) {
    const result = runScenario(scenario);
    results.push(result);
    process.stdout.write(`${formatResult(result)}\n`);
  }
  const totalMs = Date.now() - started;
  process.stdout.write(`\n${formatSummary(results, totalMs)}\n`);
  if (results.some((result) => !result.passed)) process.exitCode = 1;
}

function main(): void {
  const args = process.argv.slice(2);
  if (args[0] === 'dump') {
    if (args[1] === undefined) throw new Error('usage: dump <scenario-name>');
    dump(args[1]);
    return;
  }
  const filterFlag = args.indexOf('--only');
  const filter = filterFlag >= 0 ? args[filterFlag + 1] : undefined;
  if (!existsSync(scenarioFiles()[0] ?? '')) {
    process.stderr.write('no scenarios directory\n');
    process.exitCode = 1;
    return;
  }
  run(filter);
}

main();
