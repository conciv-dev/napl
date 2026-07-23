import { BASE_AGENT_TOOLS, BASE_EXCLUDE_DIRS, BASE_EXCLUDE_FILES, BASE_EXCLUDE_SUFFIXES } from './types.js';
import type { TargetAdapter } from './types.js';

const IDIOM_GUIDANCE = [
  'Target: a React single-page app built with Vite + React + TypeScript.',
  'If the workspace is empty, scaffold a Vite React + TypeScript project in place:',
  'package.json, index.html, vite.config.ts, tsconfig.json, tsconfig.node.json,',
  'src/main.tsx and src/App.tsx — then run "npm install".',
  'Use function components and hooks only — never class components.',
  'Use named exports for your own modules and keep components small and composable.',
  'Add test tooling: vitest, @testing-library/react, @testing-library/jest-dom, jsdom.',
  'Configure vitest to use the "jsdom" environment (test.environment) and a setup file',
  'that imports "@testing-library/jest-dom" — put this in vite.config.ts or vitest.config.ts.',
  'Write component tests with @testing-library/react that exercise every described behavior',
  '(rendering, user events via fireEvent/userEvent, and assertions on the DOM).',
  'Keep styling clean and minimal — a single plain CSS file is fine.',
  'Ensure "npx vitest run" passes from this directory before finishing.',
].join('\n');

export const reactAdapter: TargetAdapter = {
  name: 'react',
  idiomGuidance: IDIOM_GUIDANCE,
  agentTools: BASE_AGENT_TOOLS,
  attributionExcludeDirs: BASE_EXCLUDE_DIRS,
  attributionExcludeFiles: [...BASE_EXCLUDE_FILES, 'vite.config.js'],
  attributionExcludeSuffixes: BASE_EXCLUDE_SUFFIXES,
  testCommandLabel: 'npx vitest run',
  testCommand() {
    return { command: 'npx', args: ['vitest', 'run'] };
  },
};
