import { readFileSync } from 'node:fs';
import { parse as parseYaml } from 'yaml';
import type { Scenario } from './types.ts';

function asStringArray(value: unknown, label: string): string[] {
  if (!Array.isArray(value) || value.some((item) => typeof item !== 'string')) {
    throw new Error(`${label} must be an array of strings`);
  }
  return value as string[];
}

export function loadScenario(path: string): Scenario {
  const raw = readFileSync(path, 'utf8');
  const data = parseYaml(raw) as Record<string, unknown> | null;
  if (data === null || typeof data !== 'object') {
    throw new Error(`scenario ${path} is not a YAML mapping`);
  }
  if (typeof data.name !== 'string' || data.name === '') {
    throw new Error(`scenario ${path} is missing a "name"`);
  }
  if (typeof data.description !== 'string') {
    throw new Error(`scenario ${data.name} is missing a "description"`);
  }
  const run = asStringArray(data.run, `scenario ${data.name} "run"`);
  const expectRaw = data.expect as Record<string, unknown> | undefined;
  if (expectRaw === undefined || typeof expectRaw.exitCode !== 'number') {
    throw new Error(`scenario ${data.name} "expect" must include a numeric exitCode`);
  }
  return {
    name: data.name,
    description: data.description,
    setup: (data.setup as Record<string, string>) ?? undefined,
    env: (data.env as Record<string, string>) ?? undefined,
    run,
    testExit: typeof data.testExit === 'number' ? data.testExit : undefined,
    testOutput: typeof data.testOutput === 'string' ? data.testOutput : undefined,
    script: (data.script as Scenario['script']) ?? undefined,
    expect: expectRaw as unknown as Scenario['expect'],
  };
}
