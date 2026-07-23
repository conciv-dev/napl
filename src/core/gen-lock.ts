import { existsSync } from 'node:fs';
import { mkdir, readFile, rm, writeFile } from 'node:fs/promises';
import { dirname } from 'node:path';

export interface GenLockHandle {
  release(): Promise<void>;
}

export interface GenLockDeps {
  pid?: number;
  isAlive?: (pid: number) => boolean;
}

function defaultIsAlive(pid: number): boolean {
  try {
    process.kill(pid, 0);
    return true;
  } catch (error) {
    return (error as NodeJS.ErrnoException).code === 'EPERM';
  }
}

async function readHeldPid(lockPath: string): Promise<number | null> {
  try {
    const raw = (await readFile(lockPath, 'utf8')).trim();
    const pid = Number.parseInt(raw, 10);
    return Number.isNaN(pid) ? null : pid;
  } catch {
    return null;
  }
}

export async function acquireGenLock(lockPath: string, deps: GenLockDeps = {}): Promise<GenLockHandle> {
  const pid = deps.pid ?? process.pid;
  const isAlive = deps.isAlive ?? defaultIsAlive;
  await mkdir(dirname(lockPath), { recursive: true });

  try {
    await writeFile(lockPath, `${pid}\n`, { encoding: 'utf8', flag: 'wx' });
  } catch (cause) {
    if ((cause as NodeJS.ErrnoException).code !== 'EEXIST') {
      throw new Error(`could not acquire gen lock at ${lockPath}: ${(cause as Error).message}`, { cause });
    }
    const heldPid = await readHeldPid(lockPath);
    if (heldPid !== null && heldPid !== pid && isAlive(heldPid)) {
      throw new Error(
        `another hl gen is already running (pid ${heldPid}); the lock ${lockPath} is held. Wait for it to finish or remove the lock if the process is gone.`,
      );
    }
    await writeFile(lockPath, `${pid}\n`, { encoding: 'utf8', flag: 'w' });
  }

  return {
    async release(): Promise<void> {
      if (existsSync(lockPath)) await rm(lockPath, { force: true });
    },
  };
}
