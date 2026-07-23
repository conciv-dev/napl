import { BASE_AGENT_TOOLS, BASE_EXCLUDE_DIRS, BASE_EXCLUDE_FILES, BASE_EXCLUDE_SUFFIXES } from './types.js';
import type { TargetAdapter } from './types.js';

const IDIOM_GUIDANCE = [
  'Target: a TypeScript library package (ESM, strict mode) tested with vitest.',
  'If the workspace has no package.json, scaffold one in place: a "type": "module" package,',
  'add vitest and typescript as devDependencies, add a "test" script running "vitest run",',
  'and run "npm install".',
  'Write idiomatic, modern TypeScript. Use named exports only — never default exports.',
  'Prefer plain functions; never introduce a class when a function suffices.',
  'Co-locate a *.test.ts vitest suite that asserts the described behavior.',
  'Ensure "npx vitest run" passes from this directory before finishing.',
].join('\n');

export const typescriptAdapter: TargetAdapter = {
  name: 'typescript',
  idiomGuidance: IDIOM_GUIDANCE,
  agentTools: BASE_AGENT_TOOLS,
  attributionExcludeDirs: BASE_EXCLUDE_DIRS,
  attributionExcludeFiles: BASE_EXCLUDE_FILES,
  attributionExcludeSuffixes: BASE_EXCLUDE_SUFFIXES,
  testCommandLabel: 'npx vitest run',
  testCommand() {
    return { command: 'npx', args: ['vitest', 'run'] };
  },
};
