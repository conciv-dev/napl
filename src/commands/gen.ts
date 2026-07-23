import { existsSync } from 'node:fs';
import { chmod, mkdir, readFile, writeFile } from 'node:fs/promises';
import { join, relative, sep } from 'node:path';
import { parse as parseYaml, stringify as stringifyYaml } from 'yaml';
import type { AgentRunner } from '../core/agent.js';
import { validateAttribution } from '../core/attribution-schema.js';
import { parseAttributionEntries } from '../core/attribution-schema.js';
import type { Attribution } from '../core/attribution-schema.js';
import { numberLines, promptBodyLines } from '../core/body-lines.js';
import { runCommand } from '../core/exec.js';
import type { CommandResult } from '../core/exec.js';
import { parseFrontmatter } from '../core/frontmatter.js';
import { acquireGenLock } from '../core/gen-lock.js';
import { contentHash } from '../core/hash.js';
import { diffBodyLines, incrementalUnlockList, selectIntersectingEntries } from '../core/incremental.js';
import { validateIr } from '../core/ir-schema.js';
import type { LlmClient } from '../core/llm.js';
import { isPromptGenStale, readMap, recordAttribution, recordUnattributed, writeMap } from '../core/map.js';
import { extractYaml } from '../core/parse-output.js';
import { resolvePaths } from '../core/paths.js';
import { findPromptFiles } from '../core/paths.js';
import {
  ATTRIBUTION_SYSTEM,
  IR_DERIVATION_SYSTEM,
  buildAgentTask,
  buildAttributionRepair,
  buildAttributionUser,
  buildIncrementalTask,
  buildIrDerivationUser,
} from '../core/prompts.js';
import type { DepSummary } from '../core/prompts.js';
import { diffSnapshots, makeFilter, snapshotHashes } from '../core/snapshot.js';
import type { SnapshotFilter } from '../core/snapshot.js';
import { getAdapter } from '../targets/registry.js';
import type { TargetAdapter } from '../targets/types.js';

const MAX_ATTEMPTS = 3;
const READONLY_MODE = 0o444;
const WRITABLE_MODE = 0o644;
const SOURCE_EXTENSIONS = new Set(['.ts', '.tsx', '.js', '.jsx', '.css', '.html']);
const MAX_ATTRIBUTION_FILES = 24;
const MAX_FILE_LINES = 500;

export type ExecFn = (command: string, args: string[], cwd: string) => Promise<CommandResult>;

export interface GenOptions {
  root: string;
  target: string;
  agent: AgentRunner;
  llm: LlmClient;
  model: string;
  force?: boolean;
  full?: boolean;
  module?: string;
  log?: (message: string) => void;
  exec?: ExecFn;
}

export interface GenResult {
  generated: string[];
  skipped: string[];
}

function toPosix(path: string): string {
  return path.split(sep).join('/');
}

function firstMeaningfulLine(body: string): string {
  for (const line of body.split(/\r?\n/)) {
    const trimmed = line.replace(/^#+\s*/, '').trim();
    if (trimmed !== '') return trimmed.slice(0, 120);
  }
  return '(no description)';
}

async function unlockFiles(root: string, files: string[]): Promise<void> {
  for (const filePath of files) {
    const abs = join(root, filePath);
    if (!existsSync(abs)) continue;
    try {
      await chmod(abs, WRITABLE_MODE);
    } catch {
      continue;
    }
  }
}

function isSourceFile(relToTarget: string): boolean {
  const base = relToTarget.split('/').pop() ?? relToTarget;
  if (/\.config\.(t|j)sx?$/.test(base)) return false;
  const dot = base.lastIndexOf('.');
  if (dot < 0) return false;
  return SOURCE_EXTENSIONS.has(base.slice(dot));
}

interface NumberedFiles {
  text: string;
  labels: string[];
}

async function buildNumberedFiles(
  attributed: { abs: string; relToTarget: string }[],
): Promise<NumberedFiles> {
  const blocks: string[] = [];
  const labels: string[] = [];
  let count = 0;
  for (const file of attributed) {
    if (count >= MAX_ATTRIBUTION_FILES) break;
    if (!isSourceFile(file.relToTarget)) continue;
    let content: string;
    try {
      content = await readFile(file.abs, 'utf8');
    } catch {
      continue;
    }
    const lines = content.split(/\r?\n/).slice(0, MAX_FILE_LINES);
    blocks.push(`=== FILE: ${file.relToTarget} ===\n${numberLines(lines)}`);
    labels.push(file.relToTarget);
    count += 1;
  }
  return { text: blocks.join('\n\n'), labels };
}

const YAML_STRICTNESS =
  '\n\nEmit STRICTLY valid YAML. Quote every string value with double quotes and escape any ' +
  'inner double quotes, especially values containing a colon, quote, or bracket.';

async function deriveIr(
  llm: LlmClient,
  irDir: string,
  module: string,
  body: string,
  numberedFiles: string,
  log?: (message: string) => void,
): Promise<void> {
  if (numberedFiles.trim() === '') return;
  let lastError: unknown;
  for (let attempt = 1; attempt <= 2; attempt += 1) {
    try {
      const response = await llm.complete({
        system: attempt === 1 ? IR_DERIVATION_SYSTEM : IR_DERIVATION_SYSTEM + YAML_STRICTNESS,
        user: buildIrDerivationUser(module, body, numberedFiles),
      });
      const parsed = parseYaml(extractYaml(response));
      const ir = validateIr({
        ...(typeof parsed === 'object' && parsed !== null ? parsed : {}),
        module,
      });
      const irPath = join(irDir, `${module}.yaml`);
      await mkdir(join(irPath, '..'), { recursive: true });
      await writeFile(irPath, stringifyYaml(ir), 'utf8');
      return;
    } catch (cause) {
      lastError = cause;
    }
  }
  log?.(`  warn: IR derivation for '${module}' failed (best-effort, IR skipped, gen continues): ${errorMessage(lastError)}`);
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function assertAttributionSane(attribution: Attribution, allowedFiles: string[]): void {
  if (allowedFiles.length > 0 && attribution.entries.length === 0) {
    throw new Error('attribution has no entries but the module has attributed source files');
  }
  const allowed = new Set(allowedFiles);
  for (const entry of attribution.entries) {
    if (!allowed.has(entry.file)) {
      throw new Error(
        `attribution entry references file "${entry.file}" which is outside the attributed file set`,
      );
    }
  }
}

async function deriveAttributionGated(
  llm: LlmClient,
  module: string,
  target: string,
  numberedBody: string,
  numberedFiles: NumberedFiles,
  log?: (message: string) => void,
): Promise<Attribution> {
  let lastError: unknown;
  let lastOutput = '';
  for (let attempt = 1; attempt <= MAX_ATTEMPTS; attempt += 1) {
    try {
      const repair =
        attempt === 1 ? '' : buildAttributionRepair(lastOutput, errorMessage(lastError));
      const response = await llm.complete({
        system: attempt === 1 ? ATTRIBUTION_SYSTEM : ATTRIBUTION_SYSTEM + YAML_STRICTNESS,
        user: buildAttributionUser(module, numberedBody, numberedFiles.text) + repair,
      });
      lastOutput = response;
      const parsed = parseYaml(extractYaml(response));
      const entries = parseAttributionEntries(parsed);
      const attribution = validateAttribution({ module, target, entries });
      assertAttributionSane(attribution, numberedFiles.labels);
      log?.(`  attribution attempt ${attempt}/${MAX_ATTEMPTS}: ${entries.length} mapping(s) valid`);
      return attribution;
    } catch (cause) {
      lastError = cause;
      log?.(`  attribution attempt ${attempt}/${MAX_ATTEMPTS} failed: ${errorMessage(cause)}`);
    }
  }
  throw new Error(
    `attribution derivation failed for module '${module}' (${target}) after ${MAX_ATTEMPTS} attempts; last validation error: ${errorMessage(lastError)}`,
    { cause: lastError },
  );
}

async function loadPriorBody(promptsAtGenDir: string, module: string): Promise<string | null> {
  const path = join(promptsAtGenDir, `${module}.md`);
  if (!existsSync(path)) return null;
  try {
    return await readFile(path, 'utf8');
  } catch {
    return null;
  }
}

async function loadPriorAttribution(
  attributionDir: string,
  module: string,
  target: string,
): Promise<Attribution | null> {
  const path = join(attributionDir, `${module}.yaml`);
  if (!existsSync(path)) return null;
  try {
    const parsed = parseYaml(await readFile(path, 'utf8'));
    const attribution = validateAttribution(parsed);
    return attribution.target === target ? attribution : null;
  } catch {
    return null;
  }
}

async function writePriorBody(promptsAtGenDir: string, module: string, body: string): Promise<void> {
  await mkdir(promptsAtGenDir, { recursive: true });
  await writeFile(join(promptsAtGenDir, `${module}.md`), body, 'utf8');
}

async function collectSummaries(root: string, promptFiles: string[]): Promise<Map<string, DepSummary>> {
  const summaries = new Map<string, DepSummary>();
  for (const file of promptFiles) {
    const raw = await readFile(file, 'utf8');
    try {
      const { frontmatter, body } = parseFrontmatter(raw);
      summaries.set(relative(root, file), {
        module: frontmatter.module,
        summary: firstMeaningfulLine(body),
      });
    } catch {
      continue;
    }
  }
  return summaries;
}

async function runAttempts(
  adapter: TargetAdapter,
  targetDir: string,
  model: string,
  agent: AgentRunner,
  exec: ExecFn,
  task: (failure: string | null) => string,
  log?: (message: string) => void,
): Promise<boolean> {
  let failure: string | null = null;
  for (let attempt = 1; attempt <= MAX_ATTEMPTS; attempt += 1) {
    log?.(`  attempt ${attempt}/${MAX_ATTEMPTS}: running coding agent`);
    await agent.run({ task: task(failure), cwd: targetDir, model, allowedTools: adapter.agentTools });
    const { command, args } = adapter.testCommand(targetDir);
    const result = await exec(command, args, targetDir);
    if (result.code === 0) {
      log?.(`  attempt ${attempt}/${MAX_ATTEMPTS}: tests passed`);
      return true;
    }
    failure = result.output;
    log?.(`  attempt ${attempt}/${MAX_ATTEMPTS}: tests failed`);
  }
  return false;
}

export async function runGen(options: GenOptions): Promise<GenResult> {
  const paths = resolvePaths(options.root);
  const genLock = await acquireGenLock(paths.genLockPath);
  try {
    return await runGenLocked(options, paths);
  } finally {
    await genLock.release();
  }
}

async function buildTaskBuilder(
  options: GenOptions,
  paths: ReturnType<typeof resolvePaths>,
  adapter: TargetAdapter,
  target: string,
  rel: string,
  frontmatter: ReturnType<typeof parseFrontmatter>['frontmatter'],
  body: string,
  deps: DepSummary[],
  map: Awaited<ReturnType<typeof readMap>>,
  attributionDir: string,
  log?: (message: string) => void,
): Promise<{ mode: 'incremental' | 'full'; build: (failure: string | null) => string; unlock: string[] }> {
  const module = frontmatter.module;
  const targetRecord = map.prompts[rel]?.targets[target];
  const ownedFiles = targetRecord?.files ?? [];
  const canIncremental =
    options.full !== true &&
    targetRecord !== undefined &&
    targetRecord.unattributed !== true &&
    targetRecord.promptHashAtGen !== undefined;

  if (canIncremental) {
    const priorBody = await loadPriorBody(paths.promptsAtGenDir, module);
    const priorAttribution = await loadPriorAttribution(attributionDir, module, target);
    if (priorBody !== null && priorAttribution !== null) {
      const diff = diffBodyLines(priorBody, body);
      const intersectingEntries = selectIntersectingEntries(priorAttribution.entries, diff.changedOldLines);
      const targetRelToRoot = toPosix(relative(options.root, join(paths.srcDir, target)));
      const unlock = incrementalUnlockList(ownedFiles, intersectingEntries, targetRelToRoot);
      log?.(
        `  mode: INCREMENTAL — ${diff.changedOldLines.length + diff.changedNewLines.length} changed prompt line(s), ` +
          `${intersectingEntries.length} owned region(s) affected`,
      );
      const ownedRel = ownedFiles.map((filePath) => toPosix(relative(join(paths.srcDir, target), join(options.root, filePath))));
      return {
        mode: 'incremental',
        unlock,
        build: (failure) =>
          buildIncrementalTask(adapter, frontmatter, body, { module, diff: diff.unified, intersectingEntries, ownedFiles: ownedRel }, failure),
      };
    }
    log?.('  mode: full (no prior prompt body or attribution on disk to diff against)');
  } else {
    log?.(`  mode: full${options.full === true ? ' (forced --full)' : ' (no prior successful gen for this target)'}`);
  }
  return {
    mode: 'full',
    unlock: ownedFiles,
    build: (failure) => buildAgentTask(adapter, frontmatter, body, deps, failure),
  };
}

async function runGenLocked(options: GenOptions, paths: ReturnType<typeof resolvePaths>): Promise<GenResult> {
  const { root, target, agent, llm, model, force = false, log } = options;
  const exec = options.exec ?? runCommand;
  const adapter = getAdapter(target);
  const targetDir = join(paths.srcDir, target);
  const attributionDir = join(paths.hlDir, 'attribution');
  await mkdir(targetDir, { recursive: true });

  const filter: SnapshotFilter = makeFilter(
    adapter.attributionExcludeDirs,
    adapter.attributionExcludeFiles,
    adapter.attributionExcludeSuffixes,
  );
  const map = await readMap(paths.mapPath);
  const promptFiles = await findPromptFiles(root);
  const summaries = await collectSummaries(root, promptFiles);

  const generated: string[] = [];
  const skipped: string[] = [];

  for (const file of promptFiles) {
    const raw = await readFile(file, 'utf8');
    const rel = relative(root, file);
    const { frontmatter, body } = parseFrontmatter(raw);
    if (!frontmatter.targets.includes(target)) continue;

    const module = frontmatter.module;
    if (options.module !== undefined && module !== options.module) continue;

    const promptHash = contentHash(raw);
    if (!isPromptGenStale(map.prompts[rel], target, promptHash, force)) {
      skipped.push(module);
      log?.(`skip    ${module} (${target}) up to date`);
      continue;
    }

    const deps: DepSummary[] = [];
    for (const [otherRel, summary] of summaries) {
      if (otherRel === rel) continue;
      deps.push(summary);
    }

    log?.(`gen     ${module} (${target})`);
    const taskBuilder = await buildTaskBuilder(
      options,
      paths,
      adapter,
      target,
      rel,
      frontmatter,
      body,
      deps,
      map,
      attributionDir,
      log,
    );

    await unlockFiles(root, taskBuilder.unlock);

    const before = await snapshotHashes(targetDir, filter);
    const success = await runAttempts(adapter, targetDir, model, agent, exec, taskBuilder.build, log);
    if (!success) {
      throw new Error(`code generation failed for module '${module}' (${target}) after ${MAX_ATTEMPTS} attempts.`);
    }

    const after = await snapshotHashes(targetDir, filter);
    const changed = diffSnapshots(before, after);

    const attributedRel = new Set(changed.map((abs) => toPosix(relative(root, abs))));
    for (const priorPath of map.prompts[rel]?.targets[target]?.files ?? []) {
      if (existsSync(join(root, priorPath))) attributedRel.add(priorPath);
    }

    const attributed: { abs: string; relToRoot: string; relToTarget: string }[] = [];
    const files: { filePath: string; hash: string }[] = [];
    for (const relToRoot of [...attributedRel].sort()) {
      const abs = join(root, relToRoot);
      if (!existsSync(abs)) continue;
      const hash = after.get(abs) ?? contentHash(await readFile(abs, 'utf8'));
      attributed.push({ abs, relToRoot, relToTarget: toPosix(relative(targetDir, abs)) });
      files.push({ filePath: relToRoot, hash });
    }

    const numberedFiles = await buildNumberedFiles(attributed);
    const numberedBody = numberLines(promptBodyLines(raw).lines);
    await deriveIr(llm, paths.irDir, module, body, numberedFiles.text, log);

    if (numberedFiles.text.trim() === '') {
      await lockAttributed(attributed);
      recordAttribution(map, { rel, module, promptHash, target, declaredTargets: frontmatter.targets, files });
      await writePriorBody(paths.promptsAtGenDir, module, body);
      log?.(`  attributed ${files.length} file(s) to ${module}`);
      log?.('  attribution: no source files to map; span attribution skipped');
      generated.push(module);
      continue;
    }

    let attribution: Attribution;
    try {
      attribution = await deriveAttributionGated(llm, module, target, numberedBody, numberedFiles, log);
    } catch (cause) {
      recordUnattributed(map, {
        rel,
        module,
        promptHash,
        target,
        declaredTargets: frontmatter.targets,
        files: files.map((file) => file.filePath),
      });
      await writeMap(paths.mapPath, map);
      log?.(`  FAILED attribution for '${module}' (${target}); files left unlocked, target marked unattributed`);
      throw new Error(
        `gen failed for module '${module}' (${target}): required prompt attribution could not be derived after ${MAX_ATTEMPTS} attempts. The generated files were left unlocked and the target is marked unattributed; re-run 'hl gen ${target} --force' after resolving the issue. ${errorMessage(cause)}`,
        { cause },
      );
    }

    await lockAttributed(attributed);
    recordAttribution(map, { rel, module, promptHash, target, declaredTargets: frontmatter.targets, files });
    await writePriorBody(paths.promptsAtGenDir, module, body);
    const outPath = join(attributionDir, `${module}.yaml`);
    await mkdir(attributionDir, { recursive: true });
    await writeFile(outPath, stringifyYaml(attribution), 'utf8');
    log?.(`  attributed ${files.length} file(s) to ${module}`);
    log?.(`  attribution: ${attribution.entries.length} mapping(s) -> ${toPosix(relative(root, outPath))}`);

    generated.push(module);
  }

  await writeMap(paths.mapPath, map);
  return { generated, skipped };
}

async function lockAttributed(attributed: { abs: string }[]): Promise<void> {
  for (const file of attributed) {
    await chmod(file.abs, READONLY_MODE);
  }
}
