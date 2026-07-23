import { existsSync } from 'node:fs';
import { mkdir, mkdtemp, readFile, rm, stat, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { runGen } from '../src/commands/gen.js';
import type { AgentRunner } from '../src/core/agent.js';
import type { CommandResult } from '../src/core/exec.js';
import { contentHash } from '../src/core/hash.js';
import type { LlmClient } from '../src/core/llm.js';
import { emptyMap, readMap, recordAttribution, writeMap } from '../src/core/map.js';

let root: string;

const PROMPT = `---
module: greeting
deps: []
targets: [typescript]
tests: []
---
Greet a person by name.
`;

const NOOP_LLM: LlmClient = { complete: vi.fn(async () => '') };

function fakeAgent(): AgentRunner {
  return { run: vi.fn(async () => ({ output: 'agent ran', code: 0 })) };
}

async function writePrompt(): Promise<void> {
  await mkdir(join(root, 'examples'), { recursive: true });
  await writeFile(join(root, 'examples', 'greeting.hl'), PROMPT, 'utf8');
}

async function seedUpToDate(): Promise<void> {
  const map = emptyMap();
  recordAttribution(map, {
    rel: 'examples/greeting.hl',
    module: 'greeting',
    promptHash: contentHash(PROMPT),
    target: 'typescript',
    declaredTargets: ['typescript'],
    files: [],
  });
  await writeMap(join(root, '.hl', 'map.json'), map);
}

beforeEach(async () => {
  root = await mkdtemp(join(tmpdir(), 'hl-gen-'));
  await writePrompt();
});

afterEach(async () => {
  await rm(root, { recursive: true, force: true });
});

describe('runGen staleness', () => {
  it('skips an up-to-date prompt without invoking the agent or tests', async () => {
    await seedUpToDate();
    const agent = fakeAgent();
    const exec = vi.fn(async (): Promise<CommandResult> => ({ code: 0, output: '' }));

    const result = await runGen({ root, target: 'typescript', agent, llm: NOOP_LLM, model: 'm', exec });

    expect(result.skipped).toEqual(['greeting']);
    expect(result.generated).toEqual([]);
    expect(agent.run).not.toHaveBeenCalled();
    expect(exec).not.toHaveBeenCalled();
  });

  it('fails loudly after 3 attempts when tests never pass, invoking the agent each time', async () => {
    await seedUpToDate();
    const agent = fakeAgent();
    const exec = vi.fn(async (): Promise<CommandResult> => ({ code: 1, output: 'FAIL: boom' }));

    await expect(
      runGen({ root, target: 'typescript', agent, llm: NOOP_LLM, model: 'm', force: true, exec }),
    ).rejects.toThrow(/code generation failed/);

    expect(agent.run).toHaveBeenCalledTimes(3);
    expect(exec).toHaveBeenCalledTimes(3);
  });
});

describe('runGen attribution', () => {
  it('attributes agent-created files to the driving prompt, locks them, and derives artifacts', async () => {
    const targetDir = join(root, '.hl', 'src', 'typescript');
    const agent: AgentRunner = {
      run: vi.fn(async () => {
        await mkdir(targetDir, { recursive: true });
        await writeFile(join(targetDir, 'greeting.ts'), 'export const greet = (n: string) => `Hello, ${n}!`;\n', 'utf8');
        return { output: 'done', code: 0 };
      }),
    };
    const exec = vi.fn(async (): Promise<CommandResult> => ({ code: 0, output: 'ok' }));
    const complete = vi.fn(async ({ system }: { system: string }) => {
      if (system.includes('intermediate representation')) {
        return '```yaml\nmodule: greeting\ndeps: []\ntypes: []\nfunctions:\n  - name: greet\n    signature: "greet(name: string): string"\n    behavior: "Returns Hello, name!"\ntests: []\n```';
      }
      return '```yaml\n- promptLines: [1, 1]\n  file: greeting.ts\n  lines: [1, 1]\n  note: "builds the greeting"\n```';
    });
    const llm: LlmClient = { complete };

    const result = await runGen({ root, target: 'typescript', agent, llm, model: 'm', exec });

    expect(result.generated).toEqual(['greeting']);
    const map = await readMap(join(root, '.hl', 'map.json'));
    expect(map.prompts['examples/greeting.hl'].targets.typescript.files).toEqual([
      '.hl/src/typescript/greeting.ts',
    ]);
    expect(map.files['.hl/src/typescript/greeting.ts'].prompts).toEqual(['examples/greeting.hl']);

    const ir = await readFile(join(root, '.hl', 'ir', 'greeting.yaml'), 'utf8');
    expect(ir).toContain('module: greeting');
    const attribution = await readFile(join(root, '.hl', 'attribution', 'greeting.yaml'), 'utf8');
    expect(attribution).toContain('builds the greeting');
  });
});

const IR_YAML =
  '```yaml\nmodule: greeting\ndeps: []\ntypes: []\nfunctions: []\ntests: []\n```';
const VALID_ATTR =
  '```yaml\n- promptLines: [1, 1]\n  file: greeting.ts\n  lines: [1, 1]\n  note: "builds the greeting"\n```';
const MALFORMED_ATTR = '```yaml\n- promptLines: nope\n  file: greeting.ts\n  lines: [1, 1]\n```';
const EMPTY_ATTR = '```yaml\n[]\n```';
const OUT_OF_SET_ATTR =
  '```yaml\n- promptLines: [1, 1]\n  file: nowhere.ts\n  lines: [1, 1]\n  note: "x"\n```';

function writingAgent(): AgentRunner {
  return {
    run: vi.fn(async () => {
      const targetDir = join(root, '.hl', 'src', 'typescript');
      await mkdir(targetDir, { recursive: true });
      await writeFile(
        join(targetDir, 'greeting.ts'),
        'export const greet = (n: string) => `Hello, ${n}!`;\n',
        'utf8',
      );
      return { output: 'done', code: 0 };
    }),
  };
}

const PASSING_EXEC = vi.fn(async (): Promise<CommandResult> => ({ code: 0, output: 'ok' }));

function attrLlm(responses: string[]): LlmClient {
  let i = 0;
  return {
    complete: vi.fn(async ({ system }: { system: string }) => {
      if (system.includes('intermediate representation')) return IR_YAML;
      const response = responses[Math.min(i, responses.length - 1)];
      i += 1;
      return response;
    }),
  };
}

function targetFile(): string {
  return join(root, '.hl', 'src', 'typescript', 'greeting.ts');
}

async function isWritable(path: string): Promise<boolean> {
  return ((await stat(path)).mode & 0o200) !== 0;
}

describe('runGen attribution gate', () => {
  it('retries attribution and succeeds on a later attempt, locking files and clearing any marker', async () => {
    const llm = attrLlm([MALFORMED_ATTR, VALID_ATTR]);
    const result = await runGen({
      root,
      target: 'typescript',
      agent: writingAgent(),
      llm,
      model: 'm',
      exec: PASSING_EXEC,
    });

    expect(result.generated).toEqual(['greeting']);
    const map = await readMap(join(root, '.hl', 'map.json'));
    const entry = map.prompts['examples/greeting.hl'].targets.typescript;
    expect(entry.promptHashAtGen).toBe(contentHash(PROMPT));
    expect(entry.unattributed).toBeUndefined();
    expect(await isWritable(targetFile())).toBe(false);
    expect(existsSync(join(root, '.hl', 'attribution', 'greeting.yaml'))).toBe(true);
  });

  it('fails gen after 3 invalid attribution attempts, leaving files unlocked and marking the target unattributed', async () => {
    const llm = attrLlm([MALFORMED_ATTR]);
    await expect(
      runGen({ root, target: 'typescript', agent: writingAgent(), llm, model: 'm', exec: PASSING_EXEC }),
    ).rejects.toThrow(/attribution could not be derived/);

    expect(llm.complete).toHaveBeenCalledTimes(4);
    const map = await readMap(join(root, '.hl', 'map.json'));
    const entry = map.prompts['examples/greeting.hl'].targets.typescript;
    expect(entry.unattributed).toBe(true);
    expect(entry.promptHashAtGen).toBeUndefined();
    expect(entry.files).toEqual(['.hl/src/typescript/greeting.ts']);
    expect(await isWritable(targetFile())).toBe(true);
    expect(existsSync(join(root, '.hl', 'attribution', 'greeting.yaml'))).toBe(false);
  });

  it('rejects an attribution result with empty entries as a failed attempt', async () => {
    const llm = attrLlm([EMPTY_ATTR]);
    await expect(
      runGen({ root, target: 'typescript', agent: writingAgent(), llm, model: 'm', exec: PASSING_EXEC }),
    ).rejects.toThrow(/no entries/);
    const map = await readMap(join(root, '.hl', 'map.json'));
    expect(map.prompts['examples/greeting.hl'].targets.typescript.unattributed).toBe(true);
  });

  it('rejects an attribution result referencing a file outside the attributed set', async () => {
    const llm = attrLlm([OUT_OF_SET_ATTR]);
    await expect(
      runGen({ root, target: 'typescript', agent: writingAgent(), llm, model: 'm', exec: PASSING_EXEC }),
    ).rejects.toThrow(/outside the attributed file set/);
    const map = await readMap(join(root, '.hl', 'map.json'));
    expect(map.prompts['examples/greeting.hl'].targets.typescript.unattributed).toBe(true);
  });

  it('writes the prompt body to prompts-at-gen on a successful gen', async () => {
    await runGen({
      root,
      target: 'typescript',
      agent: writingAgent(),
      llm: attrLlm([VALID_ATTR]),
      model: 'm',
      exec: PASSING_EXEC,
    });
    const priorBody = await readFile(join(root, '.hl', 'prompts-at-gen', 'greeting.md'), 'utf8');
    expect(priorBody).toContain('Greet a person by name.');
  });

  it('clears the unattributed marker on a subsequent successful gen', async () => {
    await expect(
      runGen({
        root,
        target: 'typescript',
        agent: writingAgent(),
        llm: attrLlm([MALFORMED_ATTR]),
        model: 'm',
        exec: PASSING_EXEC,
      }),
    ).rejects.toThrow(/attribution could not be derived/);
    let map = await readMap(join(root, '.hl', 'map.json'));
    expect(map.prompts['examples/greeting.hl'].targets.typescript.unattributed).toBe(true);

    const result = await runGen({
      root,
      target: 'typescript',
      agent: writingAgent(),
      llm: attrLlm([VALID_ATTR]),
      model: 'm',
      force: true,
      exec: PASSING_EXEC,
    });

    expect(result.generated).toEqual(['greeting']);
    map = await readMap(join(root, '.hl', 'map.json'));
    const entry = map.prompts['examples/greeting.hl'].targets.typescript;
    expect(entry.unattributed).toBeUndefined();
    expect(entry.promptHashAtGen).toBe(contentHash(PROMPT));
    expect(await isWritable(targetFile())).toBe(false);
  });
});

const NEW_PROMPT = `---
module: greeting
deps: []
targets: [typescript]
tests: []
---
Greet a person by name loudly.
`;

const PRIOR_ATTRIBUTION = `module: greeting
target: typescript
entries:
  - promptLines: [1, 1]
    file: greeting.ts
    lines: [1, 1]
    note: greet
`;

function recordingAgent(tasks: string[]): AgentRunner {
  return {
    run: vi.fn(async (request: { task: string }) => {
      tasks.push(request.task);
      const targetDir = join(root, '.hl', 'src', 'typescript');
      await mkdir(targetDir, { recursive: true });
      await writeFile(
        join(targetDir, 'greeting.ts'),
        'export const greet = (n: string) => `HELLO, ${n}!`;\n',
        'utf8',
      );
      return { output: 'done', code: 0 };
    }),
  };
}

async function seedPriorGen(): Promise<void> {
  const targetDir = join(root, '.hl', 'src', 'typescript');
  await mkdir(targetDir, { recursive: true });
  await writeFile(join(targetDir, 'greeting.ts'), 'export const greet = (n: string) => `Hello, ${n}!`;\n', 'utf8');
  const map = emptyMap();
  recordAttribution(map, {
    rel: 'examples/greeting.hl',
    module: 'greeting',
    promptHash: contentHash(PROMPT),
    target: 'typescript',
    declaredTargets: ['typescript'],
    files: [{ filePath: '.hl/src/typescript/greeting.ts', hash: contentHash('seed') }],
  });
  await writeMap(join(root, '.hl', 'map.json'), map);
  await mkdir(join(root, '.hl', 'prompts-at-gen'), { recursive: true });
  await writeFile(join(root, '.hl', 'prompts-at-gen', 'greeting.md'), 'Greet a person by name.\n', 'utf8');
  await mkdir(join(root, '.hl', 'attribution'), { recursive: true });
  await writeFile(join(root, '.hl', 'attribution', 'greeting.yaml'), PRIOR_ATTRIBUTION, 'utf8');
}

describe('runGen incremental mode', () => {
  it('takes the incremental path when a prior body + attribution exist, feeding the agent a diff-scoped task', async () => {
    await seedPriorGen();
    await writeFile(join(root, 'examples', 'greeting.hl'), NEW_PROMPT, 'utf8');
    const tasks: string[] = [];

    const result = await runGen({
      root,
      target: 'typescript',
      agent: recordingAgent(tasks),
      llm: attrLlm([VALID_ATTR]),
      model: 'm',
      exec: PASSING_EXEC,
    });

    expect(result.generated).toEqual(['greeting']);
    expect(tasks[0]).toContain('INCREMENTAL update');
    expect(tasks[0]).toContain('-Greet a person by name.');
    expect(tasks[0]).toContain('+Greet a person by name loudly.');
    expect(tasks[0]).toContain('greeting.ts lines 1-1');

    const priorBody = await readFile(join(root, '.hl', 'prompts-at-gen', 'greeting.md'), 'utf8');
    expect(priorBody).toContain('loudly');
  });

  it('falls back to a full task when --full is passed even if prior state exists', async () => {
    await seedPriorGen();
    await writeFile(join(root, 'examples', 'greeting.hl'), NEW_PROMPT, 'utf8');
    const tasks: string[] = [];

    await runGen({
      root,
      target: 'typescript',
      agent: recordingAgent(tasks),
      llm: attrLlm([VALID_ATTR]),
      model: 'm',
      full: true,
      exec: PASSING_EXEC,
    });

    expect(tasks[0]).not.toContain('INCREMENTAL update');
    expect(tasks[0]).toContain('implementing the module');
  });

  it('falls back to full when no prior body exists on disk', async () => {
    const tasks: string[] = [];
    await runGen({
      root,
      target: 'typescript',
      agent: recordingAgent(tasks),
      llm: attrLlm([VALID_ATTR]),
      model: 'm',
      exec: PASSING_EXEC,
    });
    expect(tasks[0]).not.toContain('INCREMENTAL update');
  });
});

describe('runGen module scoping', () => {
  it('processes only the named module and leaves others untouched', async () => {
    const OTHER = `---
module: other
deps: []
targets: [typescript]
tests: []
---
Do something else.
`;
    await writeFile(join(root, 'examples', 'other.hl'), OTHER, 'utf8');
    const agent = writingAgent();

    const result = await runGen({
      root,
      target: 'typescript',
      agent,
      llm: attrLlm([VALID_ATTR]),
      model: 'm',
      module: 'greeting',
      exec: PASSING_EXEC,
    });

    expect(result.generated).toEqual(['greeting']);
    expect(result.skipped).toEqual([]);
    expect(existsSync(join(root, '.hl', 'attribution', 'other.yaml'))).toBe(false);
  });
});
