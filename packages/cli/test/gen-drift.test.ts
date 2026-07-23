import { mkdir, mkdtemp, rm, stat, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { runGen } from '../src/commands/gen.js';
import type { AgentRunner, CommandResult, JournalEntry, LlmClient } from '@napl/core';
import { appendJournalEntry, contentHash, emptyMap, filePatch, recordAttribution, writeMap } from '@napl/core';

let root: string;

const PROMPT = `---
module: greeting
deps: []
targets: [typescript]
tests: []
---
Greet a person by name.
`;
const BASELINE = 'export const greet = (name: string): string => `Hello, ${name}!`;\n';
const EDITED = 'export const greet = (name: string): string => `HELLO, ${name}!`; // hand edit\n';
const FILE = '.napl/src/typescript/greet.ts';

const PASSING_EXEC = vi.fn(async (): Promise<CommandResult> => ({ code: 0, output: 'ok' }));
const VALID_ATTR = '```yaml\n- promptLines: [1, 1]\n  file: greet.ts\n  lines: [1, 1]\n  note: "builds the greeting"\n```';
const IR_YAML = '```yaml\nmodule: greeting\ndeps: []\ntypes: []\nfunctions: []\ntests: []\n```';

function llm(): LlmClient {
  return {
    complete: vi.fn(async ({ system }: { system: string }) => {
      if (system.includes('intermediate representation')) return IR_YAML;
      if (system.includes('MACHINE LAYER')) return '```yaml\n[]\n```';
      return VALID_ATTR;
    }),
  };
}

function rewritingAgent(content: string, relPath: string = FILE): AgentRunner {
  return {
    run: vi.fn(async () => {
      await mkdir(join(root, '.napl', 'src', 'typescript'), { recursive: true });
      await writeFile(join(root, relPath), content, 'utf8');
      return { output: 'regenerated', code: 0 };
    }),
  };
}

async function seedDriftedState(onDisk: string): Promise<void> {
  await mkdir(join(root, 'examples'), { recursive: true });
  await writeFile(join(root, 'examples', 'greeting.napl'), PROMPT, 'utf8');
  await mkdir(join(root, '.napl', 'src', 'typescript'), { recursive: true });
  await writeFile(join(root, FILE), onDisk, 'utf8');

  const map = emptyMap();
  recordAttribution(map, {
    rel: 'examples/greeting.napl',
    module: 'greeting',
    promptHash: contentHash(PROMPT),
    target: 'typescript',
    declaredTargets: ['typescript'],
    files: [{ filePath: FILE, hash: contentHash(BASELINE) }],
  });
  await writeMap(join(root, '.napl', 'map.json'), map);

  const entry: JournalEntry = {
    gen: 1,
    timestamp: 't1',
    module: 'greeting',
    target: 'typescript',
    promptHash: contentHash(PROMPT),
    promptDiff: '',
    mode: 'full',
    files: [{ path: FILE, patch: filePatch(null, BASELINE), hashBefore: null, hashAfter: contentHash(BASELINE) }],
  };
  await appendJournalEntry(join(root, '.napl', 'journal.jsonl'), entry);
}

beforeEach(async () => {
  root = await mkdtemp(join(tmpdir(), 'napl-gendrift-'));
});

afterEach(async () => {
  await rm(root, { recursive: true, force: true });
});

describe('gen drift block', () => {
  it('hard-blocks gen when a locked file has drifted, printing guided output and exiting non-zero', async () => {
    await seedDriftedState(EDITED);
    const agent = rewritingAgent(BASELINE);
    const logs: string[] = [];

    await expect(
      runGen({ root, target: 'typescript', agent, llm: llm(), model: 'm', exec: PASSING_EXEC, log: (m) => logs.push(m) }),
    ).rejects.toThrow(/gen blocked/);

    expect(agent.run).not.toHaveBeenCalled();
    const report = logs.join('\n');
    expect(report).toContain('drift detected');
    expect(report).toContain('napl reconcile greeting');
    expect(report).toContain('napl gen typescript --module greeting --force');
    expect(report).toContain('+export const greet = (name: string): string => `HELLO');
  });

  it('shows hashes only when the baseline cannot be reconstructed from the journal', async () => {
    await seedDriftedState(EDITED);
    await rm(join(root, '.napl', 'journal.jsonl'), { force: true });
    const logs: string[] = [];

    await expect(
      runGen({ root, target: 'typescript', agent: rewritingAgent(BASELINE), llm: llm(), model: 'm', exec: PASSING_EXEC, log: (m) => logs.push(m) }),
    ).rejects.toThrow(/gen blocked/);
    const report = logs.join('\n');
    expect(report).toContain('comparing hashes only');
    expect(report).toContain(contentHash(EDITED));
  });

  it('--force bypasses the block, regenerates, and relocks the file 0444', async () => {
    await seedDriftedState(EDITED);
    const agent = rewritingAgent(BASELINE);

    const result = await runGen({ root, target: 'typescript', agent, llm: llm(), model: 'm', force: true, exec: PASSING_EXEC });

    expect(result.generated).toEqual(['greeting']);
    expect(agent.run).toHaveBeenCalledTimes(1);
    const mode = (await stat(join(root, FILE))).mode & 0o777;
    expect(mode).toBe(0o444);
  });

  it('does not block a scoped gen when a different module drifted', async () => {
    await seedDriftedState(EDITED);
    const OTHER = `---
module: other
deps: []
targets: [typescript]
tests: []
---
Do something else.
`;
    await writeFile(join(root, 'examples', 'other.napl'), OTHER, 'utf8');

    const otherLlm: LlmClient = {
      complete: vi.fn(async ({ system }: { system: string }) => {
        if (system.includes('intermediate representation')) return IR_YAML;
        if (system.includes('MACHINE LAYER')) return '```yaml\n[]\n```';
        return '```yaml\n- promptLines: [1, 1]\n  file: other.ts\n  lines: [1, 1]\n  note: "other"\n```';
      }),
    };

    const result = await runGen({
      root,
      target: 'typescript',
      agent: rewritingAgent('export const other = () => 1;\n', '.napl/src/typescript/other.ts'),
      llm: otherLlm,
      model: 'm',
      module: 'other',
      exec: PASSING_EXEC,
    });
    expect(result.generated).toEqual(['other']);
  });
});
