import { existsSync } from 'node:fs';
import { mkdir, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { acquireGenLock } from '../src/core/gen-lock.js';

let dir: string;
let lockPath: string;

beforeEach(async () => {
  dir = await mkdtemp(join(tmpdir(), 'hl-genlock-'));
  lockPath = join(dir, '.hl', 'gen.lock');
  await mkdir(join(dir, '.hl'), { recursive: true });
});

afterEach(async () => {
  await rm(dir, { recursive: true, force: true });
});

describe('acquireGenLock', () => {
  it('writes a lockfile containing the pid and removes it on release', async () => {
    const handle = await acquireGenLock(lockPath, { pid: 4242, isAlive: () => true });
    expect(existsSync(lockPath)).toBe(true);
    expect((await readFile(lockPath, 'utf8')).trim()).toBe('4242');
    await handle.release();
    expect(existsSync(lockPath)).toBe(false);
  });

  it('fails fast when the lock is held by a live process', async () => {
    await writeFile(lockPath, '9999\n', 'utf8');
    await expect(
      acquireGenLock(lockPath, { pid: 1, isAlive: (pid) => pid === 9999 }),
    ).rejects.toThrow(/another hl gen is already running \(pid 9999\)/);
  });

  it('steals a stale lock left by a dead process', async () => {
    await writeFile(lockPath, '9999\n', 'utf8');
    const handle = await acquireGenLock(lockPath, { pid: 5, isAlive: () => false });
    expect((await readFile(lockPath, 'utf8')).trim()).toBe('5');
    await handle.release();
  });

  it('allows re-acquiring after release', async () => {
    const first = await acquireGenLock(lockPath, { pid: 1, isAlive: () => true });
    await first.release();
    const second = await acquireGenLock(lockPath, { pid: 2, isAlive: () => true });
    expect(existsSync(lockPath)).toBe(true);
    await second.release();
  });
});
