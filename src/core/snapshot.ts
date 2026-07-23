import { readdir, readFile } from 'node:fs/promises';
import type { Dirent } from 'node:fs';
import { join } from 'node:path';
import { contentHash } from './hash.js';

async function readDirSafe(dir: string): Promise<Dirent[]> {
  try {
    return await readdir(dir, { withFileTypes: true });
  } catch {
    return [];
  }
}

export interface SnapshotFilter {
  excludeDirs: Set<string>;
  excludeFiles: Set<string>;
  excludeSuffixes: string[];
}

export function makeFilter(
  excludeDirs: string[],
  excludeFiles: string[],
  excludeSuffixes: string[] = [],
): SnapshotFilter {
  return {
    excludeDirs: new Set(excludeDirs),
    excludeFiles: new Set(excludeFiles),
    excludeSuffixes,
  };
}

function isExcludedFile(name: string, filter: SnapshotFilter): boolean {
  if (filter.excludeFiles.has(name)) return true;
  return filter.excludeSuffixes.some((suffix) => name.endsWith(suffix));
}

export async function snapshotHashes(dir: string, filter: SnapshotFilter): Promise<Map<string, string>> {
  const result = new Map<string, string>();
  async function walk(current: string): Promise<void> {
    const entries = await readDirSafe(current);
    for (const entry of entries) {
      const full = join(current, entry.name);
      if (entry.isDirectory()) {
        if (filter.excludeDirs.has(entry.name)) continue;
        await walk(full);
      } else if (entry.isFile()) {
        if (isExcludedFile(entry.name, filter)) continue;
        const content = await readFile(full, 'utf8');
        result.set(full, contentHash(content));
      }
    }
  }
  await walk(dir);
  return result;
}

export function diffSnapshots(before: Map<string, string>, after: Map<string, string>): string[] {
  const changed: string[] = [];
  for (const [path, hash] of after) {
    const prev = before.get(path);
    if (prev === undefined || prev !== hash) changed.push(path);
  }
  changed.sort();
  return changed;
}
