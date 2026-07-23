import { spawn, spawnSync } from 'node:child_process';

export interface AgentRunRequest {
  task: string;
  cwd: string;
  model: string;
  allowedTools: string[];
}

export interface AgentRunResult {
  output: string;
  code: number;
}

export interface AgentRunner {
  run(request: AgentRunRequest): Promise<AgentRunResult>;
}

export function requireClaudeAgent(env: NodeJS.ProcessEnv): void {
  const probe = spawnSync('claude', ['--version'], { stdio: 'ignore', env });
  if (probe.error !== undefined || probe.status !== 0) {
    throw new Error(
      'the "claude" CLI was not found on PATH. Install Claude Code (claude.ai/code) — hl gen runs it as a coding agent.',
      probe.error === undefined ? undefined : { cause: probe.error },
    );
  }
}

export function buildAgentArgs(model: string, allowedTools: string[]): string[] {
  return [
    '-p',
    '--output-format',
    'text',
    '--model',
    model,
    '--no-session-persistence',
    '--permission-mode',
    'acceptEdits',
    '--allowedTools',
    ...allowedTools,
  ];
}

export function createClaudeAgentRunner(): AgentRunner {
  return {
    run({ task, cwd, model, allowedTools }) {
      return new Promise<AgentRunResult>((resolve, reject) => {
        const child = spawn('claude', buildAgentArgs(model, allowedTools), {
          cwd,
          stdio: ['pipe', 'pipe', 'pipe'],
        });
        let stdout = '';
        let stderr = '';
        child.stdout.setEncoding('utf8');
        child.stderr.setEncoding('utf8');
        child.stdout.on('data', (chunk: string) => {
          stdout += chunk;
        });
        child.stderr.on('data', (chunk: string) => {
          stderr += chunk;
        });
        child.on('error', (cause) => {
          reject(new Error(`failed to spawn the "claude" agent: ${cause.message}`, { cause }));
        });
        child.on('close', (code) => {
          const combined = stderr.trim() === '' ? stdout : `${stdout}\n${stderr}`;
          resolve({ output: combined, code: code ?? 1 });
        });
        child.stdin.end(task);
      });
    },
  };
}
