import { existsSync } from 'node:fs';
import { chmod, mkdir, readFile, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import { emptyMap, writeMap } from '@napl/core';
import { DEFAULT_BACKEND, DEFAULT_MODEL, writeLock } from '@napl/core';
import { resolvePaths } from '@napl/core';
import { listTargets } from '@napl/core';
import { GUARD_DOC, GUARD_FILE_NAMES } from '@napl/core';
import { PRE_COMMIT_HOOK, PRE_COMMIT_HOOK_LINE, findGitDir } from '@napl/core';
import { CLAUDE_SETTINGS_SNIPPET, mergeClaudeSettings } from '@napl/core';

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

async function writeGuardDocs(srcDir: string, log?: (m: string) => void): Promise<void> {
  for (const target of listTargets()) {
    const targetDir = join(srcDir, target);
    await mkdir(targetDir, { recursive: true });
    for (const name of GUARD_FILE_NAMES) {
      await writeIfAbsent(join(targetDir, name), GUARD_DOC, log);
    }
  }
}

async function writeClaudeDenyRules(root: string, log?: (m: string) => void): Promise<void> {
  const settingsPath = join(root, '.claude', 'settings.json');
  const existing = existsSync(settingsPath) ? await readFile(settingsPath, 'utf8') : null;
  const merge = mergeClaudeSettings(existing);
  if (merge.action === 'create' && merge.content !== null) {
    await mkdir(join(root, '.claude'), { recursive: true });
    await writeFile(settingsPath, merge.content, 'utf8');
    log?.(`create  ${settingsPath} (deny Edit on .napl/src/**)`);
    return;
  }
  if (merge.action === 'update' && merge.content !== null) {
    await writeFile(settingsPath, merge.content, 'utf8');
    log?.(`update  ${settingsPath} (added deny rule for .napl/src/**)`);
    return;
  }
  if (merge.action === 'unchanged') {
    log?.(`exists  ${settingsPath} already denies edits to .napl/src/**`);
    return;
  }
  log?.(`note: ${settingsPath} exists but could not be safely merged — add this to it yourself:`);
  log?.(CLAUDE_SETTINGS_SNIPPET);
}

async function installPreCommitHook(root: string, log?: (m: string) => void): Promise<void> {
  const gitDir = findGitDir(root);
  if (gitDir === null) {
    log?.('note: no .git directory found — skipping pre-commit hook install (run napl init again after git init)');
    return;
  }
  const hooksDir = join(gitDir, 'hooks');
  const hookPath = join(hooksDir, 'pre-commit');
  if (existsSync(hookPath)) {
    log?.(`exists  ${hookPath} — leaving it untouched; add "${PRE_COMMIT_HOOK_LINE}" to it to gate commits on drift`);
    return;
  }
  await mkdir(hooksDir, { recursive: true });
  await writeFile(hookPath, PRE_COMMIT_HOOK, 'utf8');
  await chmod(hookPath, 0o755);
  log?.(`create  ${hookPath} (runs napl status; blocks commits on drift)`);
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

  await writeGuardDocs(paths.srcDir, log);

  await writeIfAbsent(join(paths.examplesDir, 'greeting.napl'), EXAMPLE_PROMPT, log);

  await writeClaudeDenyRules(root, log);
  await installPreCommitHook(root, log);

  log?.('initialized. Next: edit a *.napl prompt, then run "napl gen <target>" (e.g. napl gen typescript).');
  log?.('gen runs a coding agent that writes the source directly, then derives the IR and attribution.');
}
