import { existsSync } from 'node:fs';
import { mkdir, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import { emptyMap, writeMap } from '../core/map.js';
import { DEFAULT_BACKEND, DEFAULT_MODEL, writeLock } from '../core/lock.js';
import { resolvePaths } from '../core/paths.js';
import { listTargets } from '../targets/registry.js';

export interface InitOptions {
  root: string;
  log?: (message: string) => void;
}

const EXAMPLE_PROMPT = `---
module: greeting
deps: []
targets: [typescript]
tests:
  - name: greets by name
    given: { name: World }
    expect: { message: "Hello, World!" }
  - name: trims surrounding whitespace
    given: { name: "  Ada  " }
    expect: { message: "Hello, Ada!" }
---
# Greeting

Expose a \`greet\` function that takes a person's name and returns a friendly
greeting message.

- The greeting has the form \`Hello, <name>!\`.
- Leading and trailing whitespace in the name is trimmed before use.
- An empty or whitespace-only name is rejected with an error.
`;

async function writeIfAbsent(path: string, contents: string, log?: (m: string) => void): Promise<void> {
  if (existsSync(path)) {
    log?.(`exists  ${path}`);
    return;
  }
  await writeFile(path, contents, 'utf8');
  log?.(`create  ${path}`);
}

export async function runInit(options: InitOptions): Promise<void> {
  const { root, log } = options;
  const paths = resolvePaths(root);

  await mkdir(paths.irDir, { recursive: true });
  await mkdir(paths.srcDir, { recursive: true });
  await mkdir(paths.examplesDir, { recursive: true });

  if (!existsSync(paths.lockPath)) {
    await writeLock(paths.lockPath, { model: DEFAULT_MODEL, backend: DEFAULT_BACKEND });
    log?.(`create  ${paths.lockPath} (model: ${DEFAULT_MODEL}, backend: ${DEFAULT_BACKEND})`);
  } else {
    log?.(`exists  ${paths.lockPath}`);
  }

  if (!existsSync(paths.mapPath)) {
    await writeMap(paths.mapPath, emptyMap());
    log?.(`create  ${paths.mapPath}`);
  } else {
    log?.(`exists  ${paths.mapPath}`);
  }

  for (const target of listTargets()) {
    const targetDir = join(paths.srcDir, target);
    await mkdir(targetDir, { recursive: true });
  }

  await writeIfAbsent(join(paths.examplesDir, 'greeting.hl'), EXAMPLE_PROMPT, log);

  log?.('initialized. Next: edit a *.hl prompt, then run "hl gen <target>" (e.g. hl gen typescript).');
  log?.('gen runs a coding agent that writes the source directly, then derives the IR and attribution.');
}
