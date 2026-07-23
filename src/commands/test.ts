import { mkdir } from 'node:fs/promises';
import { join } from 'node:path';
import { runCommand } from '../core/exec.js';
import { resolvePaths } from '../core/paths.js';
import { getAdapter } from '../targets/registry.js';

export interface TestOptions {
  root: string;
  target: string;
  log?: (message: string) => void;
}

export interface TestResult {
  exitCode: number;
}

export async function runTest(options: TestOptions): Promise<TestResult> {
  const { root, target, log } = options;
  const adapter = getAdapter(target);
  const paths = resolvePaths(root);
  const targetDir = join(paths.srcDir, target);

  await mkdir(targetDir, { recursive: true });

  const { command, args } = adapter.testCommand(targetDir);
  log?.(`running ${command} ${args.join(' ')} (in ${targetDir})`);
  const result = await runCommand(command, args, targetDir);
  if (result.output.trim() !== '') {
    log?.(result.output.trimEnd());
  }
  return { exitCode: result.code };
}
