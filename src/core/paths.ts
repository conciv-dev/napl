import { readdir } from 'node:fs/promises';
import { join } from 'node:path';

export const PROMPT_EXTENSION = '.hl';

export interface HlPaths {
  root: string;
  hlDir: string;
  irDir: string;
  srcDir: string;
  mapPath: string;
  lockPath: string;
  genLockPath: string;
  promptsAtGenDir: string;
  examplesDir: string;
}

export function resolvePaths(root: string): HlPaths {
  const hlDir = join(root, '.hl');
  return {
    root,
    hlDir,
    irDir: join(hlDir, 'ir'),
    srcDir: join(hlDir, 'src'),
    mapPath: join(hlDir, 'map.json'),
    lockPath: join(hlDir, 'lock.json'),
    genLockPath: join(hlDir, 'gen.lock'),
    promptsAtGenDir: join(hlDir, 'prompts-at-gen'),
    examplesDir: join(root, 'examples'),
  };
}

const IGNORED_DIRS = new Set(['node_modules', '.hl', '.git']);

export async function findPromptFiles(root: string): Promise<string[]> {
  const results: string[] = [];
  async function walk(dir: string): Promise<void> {
    const entries = await readdir(dir, { withFileTypes: true });
    for (const entry of entries) {
      const full = join(dir, entry.name);
      if (entry.isDirectory()) {
        if (IGNORED_DIRS.has(entry.name)) continue;
        await walk(full);
      } else if (entry.isFile() && entry.name.endsWith(PROMPT_EXTENSION)) {
        results.push(full);
      }
    }
  }
  await walk(root);
  results.sort();
  return results;
}
