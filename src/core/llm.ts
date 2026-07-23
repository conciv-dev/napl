import { spawn, spawnSync } from 'node:child_process';
import Anthropic from '@anthropic-ai/sdk';
import type { Backend } from './lock.js';

export interface LlmRequest {
  system: string;
  user: string;
  maxTokens?: number;
}

export interface LlmClient {
  complete(request: LlmRequest): Promise<string>;
}

export interface CreateLlmClientOptions {
  backend: Backend;
  model: string;
  apiKey?: string;
}

export interface ClaudeCliError extends Error {
  stderr: string;
  exitCode: number | null;
}

export function requireApiKey(env: NodeJS.ProcessEnv): string {
  const key = env.ANTHROPIC_API_KEY;
  if (!key || key.trim() === '') {
    throw new Error(
      'ANTHROPIC_API_KEY is not set. Export it before running "hl build" or "hl gen", or set backend to "claude-cli" in .hl/lock.json.',
    );
  }
  return key;
}

export function requireClaudeCli(env: NodeJS.ProcessEnv): void {
  const probe = spawnSync('claude', ['--version'], { stdio: 'ignore', env });
  if (probe.error !== undefined || probe.status !== 0) {
    throw new Error(
      'the "claude" CLI was not found on PATH. Install Claude Code (claude.ai/code) or set backend to "anthropic-api" in .hl/lock.json.',
      probe.error === undefined ? undefined : { cause: probe.error },
    );
  }
}

function createClaudeCliError(
  message: string,
  stderr: string,
  exitCode: number | null,
  cause: unknown,
): ClaudeCliError {
  const error = new Error(message, { cause }) as ClaudeCliError;
  error.name = 'ClaudeCliError';
  error.stderr = stderr;
  error.exitCode = exitCode;
  return error;
}

export function createClaudeCliClient(model: string): LlmClient {
  return {
    complete({ system, user }) {
      return new Promise<string>((resolve, reject) => {
        const args = ['-p', '--output-format', 'text', '--model', model, '--no-session-persistence'];
        if (system.trim() !== '') {
          args.push('--system-prompt', system);
        }
        const child = spawn('claude', args, { stdio: ['pipe', 'pipe', 'pipe'] });
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
          reject(createClaudeCliError(`failed to spawn the "claude" CLI: ${cause.message}`, stderr, null, cause));
        });
        child.on('close', (code) => {
          if (code !== 0) {
            const detail = stderr.trim() === '' ? 'the claude CLI produced no stderr output' : stderr.trim();
            reject(
              createClaudeCliError(
                `the "claude" CLI exited with code ${code}: ${detail}`,
                stderr,
                code,
                new Error(detail),
              ),
            );
            return;
          }
          if (stdout.trim() === '') {
            reject(
              createClaudeCliError('the "claude" CLI returned an empty response', stderr, code, new Error('empty stdout')),
            );
            return;
          }
          resolve(stdout);
        });
        child.stdin.end(user);
      });
    },
  };
}

export function createAnthropicClient(model: string, apiKey: string): LlmClient {
  const anthropic = new Anthropic({ apiKey });
  return {
    async complete({ system, user, maxTokens = 8000 }) {
      const response = await anthropic.messages.create({
        model,
        max_tokens: maxTokens,
        system,
        messages: [{ role: 'user', content: user }],
      });
      const text = response.content
        .map((block) => (block.type === 'text' ? block.text : ''))
        .join('');
      if (text.trim() === '') {
        throw new Error('the model returned an empty response');
      }
      return text;
    },
  };
}

export function createLlmClient(options: CreateLlmClientOptions): LlmClient {
  if (options.backend === 'anthropic-api') {
    if (options.apiKey === undefined || options.apiKey.trim() === '') {
      throw new Error('the "anthropic-api" backend requires an API key');
    }
    return createAnthropicClient(options.model, options.apiKey);
  }
  return createClaudeCliClient(options.model);
}
