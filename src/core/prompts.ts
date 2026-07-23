import type { AttributionEntry } from './attribution-schema.js';
import type { Frontmatter } from './frontmatter.js';
import type { TargetAdapter } from '../targets/types.js';

export interface DepSummary {
  module: string;
  summary: string;
}

export interface IncrementalTaskInput {
  module: string;
  diff: string;
  intersectingEntries: AttributionEntry[];
  ownedFiles: string[];
}

export function buildIncrementalTask(
  adapter: TargetAdapter,
  frontmatter: Frontmatter,
  body: string,
  input: IncrementalTaskInput,
  failure: string | null,
): string {
  const parts: string[] = [];
  parts.push(
    `You are a coding agent making an INCREMENTAL update to the existing module "${frontmatter.module}"`,
    'in the current working directory. Its source code already exists and passes tests. The prompt',
    '(the durable source of truth) has CHANGED. Make the MINIMAL code edits needed to realize the',
    'prompt change — do not restructure or rewrite untouched code, do not reformat unrelated files.',
    '',
    'Target idiom guidance:',
    adapter.idiomGuidance,
    '',
    'Unified diff of the prompt change (old vs new prompt body):',
    '```diff',
    input.diff.trim(),
    '```',
  );
  if (input.intersectingEntries.length > 0) {
    parts.push(
      '',
      'The changed prompt lines are currently implemented by these owned code regions.',
      'Prefer editing exactly these regions:',
    );
    for (const entry of input.intersectingEntries) {
      const note = entry.note ? ` — ${entry.note}` : '';
      parts.push(`- ${entry.file} lines ${entry.lines[0]}-${entry.lines[1]}${note}`);
    }
  }
  if (input.ownedFiles.length > 0) {
    parts.push('', 'Files owned by this module (edit only what the change requires):');
    for (const file of input.ownedFiles) parts.push(`- ${file}`);
  }
  parts.push(
    '',
    'Full current prompt body (the target behavior after the change):',
    '"""',
    body.trim(),
    '"""',
    '',
    'Requirements:',
    `- Ensure "${adapter.testCommandLabel}" passes from this directory before you finish.`,
    '- Update the existing tests to match the new behavior; do not delete unrelated tests.',
    '- Use only functions/hooks, named exports, and no dead code.',
    '- Do not edit files under node_modules; use the package manager for dependencies.',
  );
  if (failure !== null) {
    parts.push(
      '',
      'A previous attempt FAILED its tests. Read the current files, fix the code and/or tests,',
      'and make the suite pass. Test output from the failed attempt:',
      '```',
      failure.trim().slice(-6000),
      '```',
    );
  }
  return parts.join('\n');
}

export function buildAgentTask(
  adapter: TargetAdapter,
  frontmatter: Frontmatter,
  body: string,
  deps: DepSummary[],
  failure: string | null,
): string {
  const parts: string[] = [];
  parts.push(
    `You are a coding agent implementing the module "${frontmatter.module}" as real, runnable`,
    'source code in the current working directory. Write and edit files directly; scaffold the',
    'project and install dependencies as needed. The prompt below is the durable source of truth.',
    '',
    'Target idiom guidance:',
    adapter.idiomGuidance,
    '',
    `Module: ${frontmatter.module}`,
  );
  if (frontmatter.deps.length > 0) {
    parts.push(`Declared dependencies: ${frontmatter.deps.join(', ')}`);
  }
  if (deps.length > 0) {
    parts.push('', 'Other modules in this project (for context, do not reimplement them):');
    for (const dep of deps) parts.push(`- ${dep.module}: ${dep.summary}`);
  }
  parts.push(
    '',
    'Specification to implement (implement exactly this behavior):',
    '"""',
    body.trim(),
    '"""',
    '',
    'Requirements:',
    `- Ensure "${adapter.testCommandLabel}" passes from this directory before you finish.`,
    '- Write a real test suite covering the described behavior.',
    '- Use only functions/hooks, named exports, and no dead code.',
    '- Do not edit files under node_modules; use the package manager for dependencies.',
  );
  if (failure !== null) {
    parts.push(
      '',
      'A previous attempt FAILED its tests. Read the current files, fix the code and/or tests,',
      'and make the suite pass. Test output from the failed attempt:',
      '```',
      failure.trim().slice(-6000),
      '```',
    );
  }
  return parts.join('\n');
}

export const IR_DERIVATION_SYSTEM = [
  'You derive a CONTRACT-LEVEL intermediate representation (IR) from finished source code.',
  'You are given a prompt (the contract in prose) and the source files that implement it.',
  'Produce a YAML document capturing CONTRACTS, not implementation, with these keys:',
  '- module: the exact module name provided.',
  '- deps: list of dependency module names (may be empty).',
  '- types: exported/public types as structural, language-neutral entries, each with a',
  '  "name" and a "description".',
  '- functions: the public functions/components, each with a "name", a language-neutral',
  '  "signature" string, and a "behavior" string covering pre/postconditions.',
  '- tests: the behavioral test cases as data, each with "name", "given", "expect".',
  'Do NOT include control flow, concurrency, memory idioms, or syntax trees.',
  'Output ONLY a single fenced ```yaml code block and nothing else.',
].join('\n');

export function buildIrDerivationUser(module: string, body: string, files: string): string {
  return [
    `Module name: ${module}`,
    '',
    'Prompt (the contract, in prose):',
    '"""',
    body.trim(),
    '"""',
    '',
    'Implementing source files:',
    files,
  ].join('\n');
}

export const ATTRIBUTION_SYSTEM = [
  'You map lines of a prompt (the contract, in prose) to the exact lines of generated source',
  'code that implement them. You are given the prompt body with 1-based line numbers, and each',
  'implementing source file with 1-based line numbers.',
  '',
  'Produce a YAML list. Each item is a mapping with these keys:',
  '- promptLines: [start, end]  — 1-based inclusive line range in the prompt body.',
  '- file: the source file path exactly as labelled (relative to the target src directory).',
  '- lines: [start, end]  — 1-based inclusive line range in that file.',
  '- note: a short phrase describing what this code does (e.g. "trims whitespace").',
  '',
  'A prompt line may map to multiple code ranges, and one code range may map to multiple',
  'prompt lines — emit one item per concrete mapping. Only map lines that carry real intent;',
  'skip blank lines, headings, and boilerplate. Keep ranges tight.',
  'Output ONLY a single fenced ```yaml code block containing the list, and nothing else.',
].join('\n');

export function buildAttributionUser(module: string, numberedBody: string, files: string): string {
  return [
    `Module: ${module}`,
    '',
    'Prompt body (1-based line numbers):',
    numberedBody,
    '',
    'Implementing source files (1-based line numbers):',
    files,
  ].join('\n');
}

export function buildAttributionRepair(previousOutput: string, errorMessage: string): string {
  return [
    '',
    'Your PREVIOUS response was REJECTED as invalid and MUST be corrected. Return ONLY a single',
    'fenced ```yaml code block containing the list — no prose, no commentary, nothing else. Every',
    'item must reference a "file" that appears EXACTLY as labelled above, and the list must not be',
    'empty. Emit STRICTLY valid YAML with double-quoted string values.',
    '',
    'Your previous invalid response was:',
    '"""',
    previousOutput.trim().slice(0, 4000),
    '"""',
    '',
    'The validation error was:',
    errorMessage,
  ].join('\n');
}
