import { readFile } from 'node:fs/promises';
import { relative } from 'node:path';
import { readMap } from '../core/map.js';
import { classifyPrompt } from '../core/status-core.js';
import type { FileStatus, StatusEntry } from '../core/status-core.js';
import { findPromptFiles, resolvePaths } from '../core/paths.js';

export type { FileStatus, StatusEntry };

export interface StatusOptions {
  root: string;
  log?: (message: string) => void;
}

export interface StatusResult {
  entries: StatusEntry[];
  exitCode: number;
}

export async function runStatus(options: StatusOptions): Promise<StatusResult> {
  const { root, log } = options;
  const paths = resolvePaths(root);
  const map = await readMap(paths.mapPath);
  const promptFiles = await findPromptFiles(root);

  const entries: StatusEntry[] = [];
  let anyError = false;

  for (const file of promptFiles) {
    const rel = relative(root, file);
    const raw = await readFile(file, 'utf8');
    const entry = await classifyPrompt({ root, relPath: rel, raw, map });
    if (entry.status === 'DRIFT' || entry.status === 'unattributed') anyError = true;
    entries.push(entry);
    const suffix = entry.detail ? ` (${entry.detail})` : '';
    log?.(`${entry.status.padEnd(12)} ${rel}${suffix}`);
  }

  return { entries, exitCode: anyError ? 1 : 0 };
}
