import { spawnSync } from 'node:child_process';
import { existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, realpathSync, rmSync, statSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { delimiter, dirname, join, relative, sep } from 'node:path';
import { fileURLToPath } from 'node:url';
import { applyTemplate, normalizePaths } from './template.ts';
import type { Scenario } from './types.ts';

const HERE = dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = join(HERE, '..', '..');
const CLI_PATH = join(REPO_ROOT, 'packages', 'cli', 'dist', 'cli.js');
const FAKE_BIN = join(REPO_ROOT, 'conformance', 'fake-claude');
const DEFAULT_NOW = '2026-07-23T00:00:00.000Z';

export interface ActualFile {
  exists: boolean;
  content: string;
  mode: string;
}

export interface ActualResult {
  exitCode: number;
  stdout: string;
  stderr: string;
  files: Map<string, ActualFile>;
  agentInputs: string[];
  workdir: string;
  allFiles?: Map<string, ActualFile>;
}

export interface ExecuteOptions {
  collectAll?: boolean;
}

const WALK_SKIP = new Set(['node_modules', '.git']);

function walkFiles(base: string, dir: string, out: Map<string, ActualFile>): void {
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const abs = join(dir, entry.name);
    if (entry.isDirectory()) {
      if (WALK_SKIP.has(entry.name)) continue;
      walkFiles(base, abs, out);
    } else if (entry.isFile()) {
      const rel = relative(base, abs).split(sep).join('/');
      out.set(rel, { exists: true, content: readFileSync(abs, 'utf8'), mode: octalMode(abs) });
    }
  }
}

export function cliPath(): string {
  return CLI_PATH;
}

function writeSetup(workdir: string, setup: Record<string, string> | undefined, cwd: string, runnerPid: number): void {
  if (setup === undefined) return;
  for (const [rel, rawContent] of Object.entries(setup)) {
    const abs = join(workdir, rel);
    mkdirSync(dirname(abs), { recursive: true });
    writeFileSync(abs, applyTemplate(rawContent, { cwd, runnerPid }), 'utf8');
  }
}

function octalMode(abs: string): string {
  return (statSync(abs).mode & 0o777).toString(8).padStart(4, '0');
}

function readActualFile(workdir: string, rel: string): ActualFile {
  const abs = join(workdir, rel);
  if (!existsSync(abs)) return { exists: false, content: '', mode: '' };
  return { exists: true, content: readFileSync(abs, 'utf8'), mode: octalMode(abs) };
}

function collectAgentInputs(captureDir: string): string[] {
  const inputs: string[] = [];
  for (let index = 1; ; index += 1) {
    const path = join(captureDir, `agent-input-${index}.txt`);
    if (!existsSync(path)) break;
    inputs.push(readFileSync(path, 'utf8'));
  }
  return inputs;
}

export function executeScenario(scenario: Scenario, options: ExecuteOptions = {}): ActualResult {
  const workdir = mkdtempSync(join(tmpdir(), 'napl-conf-'));
  const realWork = realpathSync(workdir);
  const harness = mkdtempSync(join(tmpdir(), 'napl-conf-harness-'));
  const runnerPid = process.pid;
  try {
    writeSetup(realWork, scenario.setup, realWork, runnerPid);

    const scriptPath = join(harness, 'script.json');
    writeFileSync(scriptPath, JSON.stringify(scenario.script ?? {}), 'utf8');
    const statePath = join(harness, 'state.json');
    const captureDir = join(harness, 'capture');
    mkdirSync(captureDir, { recursive: true });

    const env: NodeJS.ProcessEnv = {
      ...process.env,
      PATH: `${FAKE_BIN}${delimiter}${process.env.PATH ?? ''}`,
      NAPL_FIXED_NOW: DEFAULT_NOW,
      NAPL_FAKE_CLAUDE_SCRIPT: scriptPath,
      NAPL_FAKE_CLAUDE_STATE: statePath,
      NAPL_FAKE_CLAUDE_CAPTURE: captureDir,
      NAPL_FAKE_TEST_EXIT: String(scenario.testExit ?? 0),
      NAPL_FAKE_TEST_OUTPUT: scenario.testOutput ?? '',
      ...(scenario.env ?? {}),
    };
    for (const [key, value] of Object.entries(scenario.env ?? {})) {
      env[key] = applyTemplate(value, { cwd: realWork, runnerPid });
    }

    const proc = spawnSync('node', [CLI_PATH, ...scenario.run], {
      cwd: realWork,
      env,
      encoding: 'utf8',
      stdio: ['ignore', 'pipe', 'pipe'],
    });

    const stdout = normalizePaths(proc.stdout ?? '', workdir, realWork);
    const stderr = normalizePaths(proc.stderr ?? '', workdir, realWork);

    const files = new Map<string, ActualFile>();
    for (const rel of Object.keys(scenario.expect.files ?? {})) {
      files.set(rel, readActualFile(realWork, rel));
    }

    let allFiles: Map<string, ActualFile> | undefined;
    if (options.collectAll === true) {
      allFiles = new Map<string, ActualFile>();
      walkFiles(realWork, realWork, allFiles);
    }

    return {
      exitCode: proc.status ?? 0,
      stdout,
      stderr,
      files,
      agentInputs: collectAgentInputs(captureDir),
      workdir: realWork,
      allFiles,
    };
  } finally {
    rmSync(workdir, { recursive: true, force: true });
    rmSync(harness, { recursive: true, force: true });
  }
}
