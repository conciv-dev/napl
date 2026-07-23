import { existsSync } from 'node:fs';
import { mkdir, mkdtemp, readFile, rm, stat, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { runGen } from '../src/commands/gen.js';
import type { AgentRunner } from '@napl/core';
import type { CommandResult } from '@napl/core';
import { contentHash } from '@napl/core';
import type { LlmClient } from '@napl/core';
import { emptyMap, readMap, recordAttribution, writeMap } from '@napl/core';

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
  await writeFile(join(root, 'examples', 'greeting.napl'), PROMPT, 'utf8');
}

async function seedUpToDate(): Promise<void> {
  const map = emptyMap();
  recordAttribution(map, {
    rel: 'examples/greeting.napl',
    module: 'greeting',
    promptHash: contentHash(PROMPT),
    target: 'typescript',
    declaredTargets: ['typescript'],
    files: [],
  });
  await writeMap(join(root, '.napl', 'map.json'), map);
}

beforeEach(async () => {
  root = await mkdtemp(join(tmpdir(), 'napl-gen-'));
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
    const targetDir = join(root, '.napl', 'src', 'typescript');
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
    const map = await readMap(join(root, '.napl', 'map.json'));
    expect(map.prompts['examples/greeting.napl'].targets.typescript.files).toEqual([
      '.napl/src/typescript/greeting.ts',
    ]);
    expect(map.files['.napl/src/typescript/greeting.ts'].prompts).toEqual(['examples/greeting.napl']);

    const ir = await readFile(join(root, '.napl', 'ir', 'greeting.yaml'), 'utf8');
    expect(ir).toContain('module: greeting');
    const attribution = await readFile(join(root, '.napl', 'attribution', 'greeting.yaml'), 'utf8');
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
      const targetDir = join(root, '.napl', 'src', 'typescript');
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
  return join(root, '.napl', 'src', 'typescript', 'greeting.ts');
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
    const map = await readMap(join(root, '.napl', 'map.json'));
    const entry = map.prompts['examples/greeting.napl'].targets.typescript;
    expect(entry.promptHashAtGen).toBe(contentHash(PROMPT));
    expect(entry.unattributed).toBeUndefined();
    expect(await isWritable(targetFile())).toBe(false);
    expect(existsSync(join(root, '.napl', 'attribution', 'greeting.yaml'))).toBe(true);
  });

  it('fails gen after 3 invalid attribution attempts, leaving files unlocked and marking the target unattributed', async () => {
    const llm = attrLlm([MALFORMED_ATTR]);
    await expect(
      runGen({ root, target: 'typescript', agent: writingAgent(), llm, model: 'm', exec: PASSING_EXEC }),
    ).rejects.toThrow(/attribution could not be derived/);

    expect(llm.complete).toHaveBeenCalledTimes(4);
    const map = await readMap(join(root, '.napl', 'map.json'));
    const entry = map.prompts['examples/greeting.napl'].targets.typescript;
    expect(entry.unattributed).toBe(true);
    expect(entry.promptHashAtGen).toBeUndefined();
    expect(entry.files).toEqual(['.napl/src/typescript/greeting.ts']);
    expect(await isWritable(targetFile())).toBe(true);
    expect(existsSync(join(root, '.napl', 'attribution', 'greeting.yaml'))).toBe(false);
  });

  it('rejects an attribution result with empty entries as a failed attempt', async () => {
    const llm = attrLlm([EMPTY_ATTR]);
    await expect(
      runGen({ root, target: 'typescript', agent: writingAgent(), llm, model: 'm', exec: PASSING_EXEC }),
    ).rejects.toThrow(/no entries/);
    const map = await readMap(join(root, '.napl', 'map.json'));
    expect(map.prompts['examples/greeting.napl'].targets.typescript.unattributed).toBe(true);
  });

  it('rejects an attribution result referencing a file outside the attributed set', async () => {
    const llm = attrLlm([OUT_OF_SET_ATTR]);
    await expect(
      runGen({ root, target: 'typescript', agent: writingAgent(), llm, model: 'm', exec: PASSING_EXEC }),
    ).rejects.toThrow(/outside the attributed file set/);
    const map = await readMap(join(root, '.napl', 'map.json'));
    expect(map.prompts['examples/greeting.napl'].targets.typescript.unattributed).toBe(true);
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
    const priorBody = await readFile(join(root, '.napl', 'prompts-at-gen', 'greeting.md'), 'utf8');
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
    let map = await readMap(join(root, '.napl', 'map.json'));
    expect(map.prompts['examples/greeting.napl'].targets.typescript.unattributed).toBe(true);

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
    map = await readMap(join(root, '.napl', 'map.json'));
    const entry = map.prompts['examples/greeting.napl'].targets.typescript;
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
      const targetDir = join(root, '.napl', 'src', 'typescript');
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

const SEED_SRC = 'export const greet = (n: string) => `Hello, ${n}!`;\n';

async function seedPriorGen(): Promise<void> {
  const targetDir = join(root, '.napl', 'src', 'typescript');
  await mkdir(targetDir, { recursive: true });
  await writeFile(join(targetDir, 'greeting.ts'), SEED_SRC, 'utf8');
  const map = emptyMap();
  recordAttribution(map, {
    rel: 'examples/greeting.napl',
    module: 'greeting',
    promptHash: contentHash(PROMPT),
    target: 'typescript',
    declaredTargets: ['typescript'],
    files: [{ filePath: '.napl/src/typescript/greeting.ts', hash: contentHash(SEED_SRC) }],
  });
  await writeMap(join(root, '.napl', 'map.json'), map);
  await mkdir(join(root, '.napl', 'prompts-at-gen'), { recursive: true });
  await writeFile(join(root, '.napl', 'prompts-at-gen', 'greeting.md'), 'Greet a person by name.\n', 'utf8');
  await mkdir(join(root, '.napl', 'attribution'), { recursive: true });
  await writeFile(join(root, '.napl', 'attribution', 'greeting.yaml'), PRIOR_ATTRIBUTION, 'utf8');
}

describe('runGen incremental mode', () => {
  it('takes the incremental path when a prior body + attribution exist, feeding the agent a diff-scoped task', async () => {
    await seedPriorGen();
    await writeFile(join(root, 'examples', 'greeting.napl'), NEW_PROMPT, 'utf8');
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

    const priorBody = await readFile(join(root, '.napl', 'prompts-at-gen', 'greeting.md'), 'utf8');
    expect(priorBody).toContain('loudly');
  });

  it('falls back to a full task when --full is passed even if prior state exists', async () => {
    await seedPriorGen();
    await writeFile(join(root, 'examples', 'greeting.napl'), NEW_PROMPT, 'utf8');
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

const NOOP_ML =
  '```yaml\n- promptLines: [1, 1]\n  kind: no-op\n  message: "requirement already satisfied"\n  reasoning: "the existing code already implements this"\n```';
const NOTE_ONLY_ML =
  '```yaml\n- promptLines: [1, 1]\n  kind: note\n  message: "nothing to add"\n  reasoning: "x"\n```';
const EMPTY_ML = '```yaml\n[]\n```';
const BAD_ML = '```yaml\n- promptLines: nope\n  kind: mystery\n  message: "x"\n```';

function noopAgent(tasks: string[]): AgentRunner {
  return {
    run: vi.fn(async (request: { task: string }) => {
      tasks.push(request.task);
      return { output: 'I made no source changes.', code: 0 };
    }),
  };
}

function mlLlm(mlResponses: string[]): LlmClient {
  let i = 0;
  return {
    complete: vi.fn(async ({ system }: { system: string }) => {
      if (system.includes('intermediate representation')) return IR_YAML;
      if (system.includes('MACHINE LAYER')) {
        const response = mlResponses[Math.min(i, mlResponses.length - 1)];
        i += 1;
        return response;
      }
      return VALID_ATTR;
    }),
  };
}

describe('runGen no-op rule (the silent-success bug fix)', () => {
  it('changed prompt + empty diff + valid no-op note → success WITH ml entry, module stays clean', async () => {
    await seedPriorGen();
    await writeFile(join(root, 'examples', 'greeting.napl'), NEW_PROMPT, 'utf8');
    const tasks: string[] = [];
    const agent = noopAgent(tasks);

    const result = await runGen({
      root,
      target: 'typescript',
      agent,
      llm: mlLlm([NOOP_ML]),
      model: 'm',
      exec: PASSING_EXEC,
    });

    expect(result.generated).toEqual(['greeting']);
    expect(agent.run).toHaveBeenCalledTimes(2);
    expect(tasks[1]).toContain('CRITICAL');

    const map = await readMap(join(root, '.napl', 'map.json'));
    const entry = map.prompts['examples/greeting.napl'].targets.typescript;
    expect(entry.promptHashAtGen).toBe(contentHash(NEW_PROMPT));

    const ml = await readFile(join(root, '.napl', 'mapl', 'greeting.mapl'), 'utf8');
    expect(ml).toContain('kind: no-op');
    expect(ml).toContain('requirement already satisfied');
  });

  it('changed prompt + empty diff + no no-op entry → gen FAILS and module stays stale', async () => {
    await seedPriorGen();
    await writeFile(join(root, 'examples', 'greeting.napl'), NEW_PROMPT, 'utf8');

    await expect(
      runGen({
        root,
        target: 'typescript',
        agent: noopAgent([]),
        llm: mlLlm([NOTE_ONLY_ML]),
        model: 'm',
        exec: PASSING_EXEC,
      }),
    ).rejects.toThrow(/made no source edits/);

    const map = await readMap(join(root, '.napl', 'map.json'));
    const entry = map.prompts['examples/greeting.napl'].targets.typescript;
    expect(entry.promptHashAtGen).toBe(contentHash(PROMPT));
    expect(entry.promptHashAtGen).not.toBe(contentHash(NEW_PROMPT));
    const ml = await readFile(join(root, '.napl', 'mapl', 'greeting.mapl'), 'utf8');
    expect(ml).toContain('kind: note');
  });

  it('changed prompt + empty diff + failed ml derivation → gen FAILS and stays stale', async () => {
    await seedPriorGen();
    await writeFile(join(root, 'examples', 'greeting.napl'), NEW_PROMPT, 'utf8');

    await expect(
      runGen({
        root,
        target: 'typescript',
        agent: noopAgent([]),
        llm: mlLlm([BAD_ML]),
        model: 'm',
        exec: PASSING_EXEC,
      }),
    ).rejects.toThrow(/machine-layer derivation failed/);

    const map = await readMap(join(root, '.napl', 'map.json'));
    expect(map.prompts['examples/greeting.napl'].targets.typescript.promptHashAtGen).toBe(contentHash(PROMPT));
  });

  it('unchanged prompt + --force + empty diff → old idempotent success, no no-op required', async () => {
    await seedPriorGen();
    const tasks: string[] = [];
    const agent = noopAgent(tasks);

    const result = await runGen({
      root,
      target: 'typescript',
      agent,
      llm: mlLlm([EMPTY_ML]),
      model: 'm',
      force: true,
      exec: PASSING_EXEC,
    });

    expect(result.generated).toEqual(['greeting']);
    expect(agent.run).toHaveBeenCalledTimes(1);
    expect(tasks.some((task) => task.includes('CRITICAL'))).toBe(false);
    const map = await readMap(join(root, '.napl', 'map.json'));
    expect(map.prompts['examples/greeting.napl'].targets.typescript.promptHashAtGen).toBe(contentHash(PROMPT));
    expect(existsSync(join(root, '.napl', 'mapl', 'greeting.mapl'))).toBe(true);
  });
});

const EMOJI_PROMPT = `---
module: greeting
deps: []
targets: [typescript]
tests: []
---
Greet a person by name.
`;

describe('runGen machine-file mirror rule', () => {
  it('writes the machine file as .🤖 when the prompt uses an emoji alias, not .mapl', async () => {
    await rm(join(root, 'examples', 'greeting.napl'), { force: true });
    await writeFile(join(root, 'examples', 'greeting.🧑'), EMOJI_PROMPT, 'utf8');

    const result = await runGen({
      root,
      target: 'typescript',
      agent: writingAgent(),
      llm: attrLlm([VALID_ATTR]),
      model: 'm',
      exec: PASSING_EXEC,
    });

    expect(result.generated).toEqual(['greeting']);
    expect(existsSync(join(root, '.napl', 'mapl', 'greeting.🤖'))).toBe(true);
    expect(existsSync(join(root, '.napl', 'mapl', 'greeting.mapl'))).toBe(false);

    const map = await readMap(join(root, '.napl', 'map.json'));
    expect(map.prompts['examples/greeting.🧑'].targets.typescript.files).toEqual([
      '.napl/src/typescript/greeting.ts',
    ]);
  });

  it('keeps the canonical .mapl machine file for a canonical .napl prompt', async () => {
    const result = await runGen({
      root,
      target: 'typescript',
      agent: writingAgent(),
      llm: attrLlm([VALID_ATTR]),
      model: 'm',
      exec: PASSING_EXEC,
    });

    expect(result.generated).toEqual(['greeting']);
    expect(existsSync(join(root, '.napl', 'mapl', 'greeting.mapl'))).toBe(true);
    expect(existsSync(join(root, '.napl', 'mapl', 'greeting.🤖'))).toBe(false);
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
    await writeFile(join(root, 'examples', 'other.napl'), OTHER, 'utf8');
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
    expect(existsSync(join(root, '.napl', 'attribution', 'other.yaml'))).toBe(false);
  });
});
