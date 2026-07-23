import { existsSync } from 'node:fs';
import { readFile } from 'node:fs/promises';
import { join } from 'node:path';
import { parseFrontmatter } from './frontmatter.js';
import { contentHash } from './hash.js';
import type { HlMap, PromptRecord } from './map.js';

export type FileStatus = 'clean' | 'prompt-stale' | 'DRIFT' | 'unattributed';

export interface StatusEntry {
  file: string;
  module: string;
  status: FileStatus;
  detail: string;
  driftFile?: string;
}

export interface ClassifyInput {
  root: string;
  relPath: string;
  raw: string;
  map: HlMap;
}

interface DriftResult {
  drift: boolean;
  detail: string;
  driftFile?: string;
}

async function detectDrift(root: string, record: PromptRecord, map: HlMap): Promise<DriftResult> {
  for (const [target, targetRecord] of Object.entries(record.targets)) {
    if (targetRecord.unattributed === true) continue;
    for (const filePath of targetRecord.files) {
      const abs = join(root, filePath);
      if (!existsSync(abs)) {
        return { drift: true, detail: `${target}: ${filePath} is missing`, driftFile: filePath };
      }
      const expected = map.files[filePath]?.hash;
      const actual = contentHash(await readFile(abs, 'utf8'));
      if (expected === undefined || actual !== expected) {
        return { drift: true, detail: `${target}: ${filePath} was edited`, driftFile: filePath };
      }
    }
  }
  return { drift: false, detail: '' };
}

function detectUnattributed(record: PromptRecord): string | null {
  for (const [target, targetRecord] of Object.entries(record.targets)) {
    if (targetRecord.unattributed === true) {
      return `generated files lack prompt attribution — run hl gen ${target} --force`;
    }
  }
  return null;
}

function detectPromptStale(record: PromptRecord, declaredTargets: string[], promptHash: string): string | null {
  for (const target of declaredTargets) {
    const targetRecord = record.targets[target];
    if (targetRecord === undefined) return `${target}: not generated`;
    if (targetRecord.promptHashAtGen !== promptHash) return 'prompt changed since gen';
  }
  return null;
}

export async function classifyPrompt(input: ClassifyInput): Promise<StatusEntry> {
  const { root, relPath, raw, map } = input;
  const { frontmatter } = parseFrontmatter(raw);
  const module = frontmatter.module;
  const promptHash = contentHash(raw);
  const record = map.prompts[relPath];

  if (record !== undefined) {
    const drift = await detectDrift(root, record, map);
    if (drift.drift) {
      return { file: relPath, module, status: 'DRIFT', detail: drift.detail, driftFile: drift.driftFile };
    }
    const unattributed = detectUnattributed(record);
    if (unattributed !== null) {
      return { file: relPath, module, status: 'unattributed', detail: unattributed };
    }
  }

  if (record === undefined) {
    return { file: relPath, module, status: 'prompt-stale', detail: 'never generated' };
  }

  const staleDetail = detectPromptStale(record, frontmatter.targets, promptHash);
  if (staleDetail !== null) {
    return { file: relPath, module, status: 'prompt-stale', detail: staleDetail };
  }

  return { file: relPath, module, status: 'clean', detail: '' };
}
