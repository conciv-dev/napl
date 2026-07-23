import { existsSync } from 'node:fs';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { dirname } from 'node:path';
import { z } from 'zod';

export const DEFAULT_MODEL = 'claude-sonnet-5';

export const backendSchema = z.enum(['claude-cli', 'anthropic-api']);

export type Backend = z.infer<typeof backendSchema>;

export const DEFAULT_BACKEND: Backend = 'claude-cli';

export const lockSchema = z.object({
  model: z.string().min(1),
  backend: backendSchema.default(DEFAULT_BACKEND),
});

export type HlLock = z.infer<typeof lockSchema>;

export function parseLock(raw: string): HlLock {
  let data: unknown;
  try {
    data = JSON.parse(raw);
  } catch (cause) {
    throw new Error('corrupt lock.json', { cause });
  }
  const parsed = lockSchema.safeParse(data);
  if (!parsed.success) {
    throw new Error(`invalid lock.json: ${parsed.error.message}`, { cause: parsed.error });
  }
  return parsed.data;
}

export async function readLock(lockPath: string): Promise<HlLock> {
  if (!existsSync(lockPath)) {
    throw new Error("missing .hl/lock.json — run 'hl init' first");
  }
  return parseLock(await readFile(lockPath, 'utf8'));
}

export async function writeLock(lockPath: string, lock: HlLock): Promise<void> {
  await mkdir(dirname(lockPath), { recursive: true });
  await writeFile(lockPath, `${JSON.stringify(lock, null, 2)}\n`, 'utf8');
}
