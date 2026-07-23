#!/usr/bin/env node
import { Command } from 'commander';
import { createClaudeAgentRunner, requireClaudeAgent } from './core/agent.js';
import type { AgentRunner } from './core/agent.js';
import { runBuild } from './commands/build.js';
import { runGen } from './commands/gen.js';
import { runInit } from './commands/init.js';
import { runStatus } from './commands/status.js';
import { runTest } from './commands/test.js';
import { createLlmClient, requireApiKey, requireClaudeCli } from './core/llm.js';
import { readLock } from './core/lock.js';
import { resolvePaths } from './core/paths.js';
import type { LlmClient } from './core/llm.js';

async function makeLlm(root: string): Promise<{ llm: LlmClient; model: string }> {
  const paths = resolvePaths(root);
  const lock = await readLock(paths.lockPath);
  if (lock.backend === 'anthropic-api') {
    const apiKey = requireApiKey(process.env);
    return { llm: createLlmClient({ backend: 'anthropic-api', model: lock.model, apiKey }), model: lock.model };
  }
  requireClaudeCli(process.env);
  return { llm: createLlmClient({ backend: 'claude-cli', model: lock.model }), model: lock.model };
}

function makeAgent(): AgentRunner {
  requireClaudeAgent(process.env);
  return createClaudeAgentRunner();
}

const program = new Command();

program
  .name('hl')
  .description('human-language: prompts are the source of truth; a coding agent generates and locks target code')
  .version('0.1.0');

program
  .command('init')
  .description('create the .hl/ structure, lock.json, and an example prompt')
  .action(async () => {
    await runInit({ root: process.cwd(), log: (m) => console.log(m) });
  });

program
  .command('build')
  .description('deprecated — gen now works directly from prompts')
  .action(async () => {
    await runBuild({ log: (m) => console.log(m) });
  });

program
  .command('gen')
  .description('run a coding agent that writes target code from prompts, run its tests, then lock and derive IR + attribution')
  .argument('<target>', 'target language (e.g. typescript, react)')
  .option('-f, --force', 'regenerate every prompt even when the prompt has not changed')
  .option('--full', 'force from-scratch generation instead of the automatic incremental (diff-scoped) mode')
  .option('-m, --module <module>', 'scope the run to a single module by name')
  .action(async (target: string, opts: { force?: boolean; full?: boolean; module?: string }) => {
    const { llm, model } = await makeLlm(process.cwd());
    const agent = makeAgent();
    const result = await runGen({
      root: process.cwd(),
      target,
      agent,
      llm,
      model,
      force: opts.force ?? false,
      full: opts.full ?? false,
      module: opts.module,
      log: (m) => console.log(m),
    });
    console.log(`generated ${result.generated.length}, skipped ${result.skipped.length}`);
  });

program
  .command('status')
  .description('report clean / prompt-stale / DRIFT / unattributed per prompt (exit 1 on drift or unattributed)')
  .action(async () => {
    const { exitCode } = await runStatus({ root: process.cwd(), log: (m) => console.log(m) });
    process.exitCode = exitCode;
  });

program
  .command('test')
  .description('run generated tests for a target without regenerating')
  .argument('[target]', 'target language', 'typescript')
  .action(async (target: string) => {
    const { exitCode } = await runTest({ root: process.cwd(), target, log: (m) => console.log(m) });
    process.exitCode = exitCode;
  });

program.parseAsync(process.argv).catch((err: unknown) => {
  const message = err instanceof Error ? err.message : String(err);
  console.error(`hl: ${message}`);
  process.exitCode = 1;
});
