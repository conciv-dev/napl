import { EventEmitter } from 'node:events';
import { afterEach, describe, expect, it, vi } from 'vitest';

const spawn = vi.fn();

vi.mock('node:child_process', () => ({
  spawn: (...args: unknown[]) => spawn(...args),
  spawnSync: vi.fn(),
}));

const { createClaudeCliClient, createLlmClient } = await import('../src/core/llm.js');

interface FakeChild extends EventEmitter {
  stdout: EventEmitter & { setEncoding: (encoding: string) => void };
  stderr: EventEmitter & { setEncoding: (encoding: string) => void };
  stdin: { end: (data: string) => void };
}

function makeFakeChild(): { child: FakeChild; stdinEnd: ReturnType<typeof vi.fn> } {
  const stdout = Object.assign(new EventEmitter(), { setEncoding: () => {} });
  const stderr = Object.assign(new EventEmitter(), { setEncoding: () => {} });
  const stdinEnd = vi.fn();
  const child = Object.assign(new EventEmitter(), {
    stdout,
    stderr,
    stdin: { end: stdinEnd },
  }) as FakeChild;
  return { child, stdinEnd };
}

afterEach(() => {
  spawn.mockReset();
});

describe('createClaudeCliClient', () => {
  it('spawns claude with the expected flags, pipes the prompt via stdin, and returns stdout', async () => {
    const { child, stdinEnd } = makeFakeChild();
    spawn.mockReturnValue(child);

    const client = createClaudeCliClient('claude-sonnet-5');
    const promise = client.complete({ system: 'you are a compiler', user: 'compile this' });

    expect(spawn).toHaveBeenCalledWith(
      'claude',
      [
        '-p',
        '--output-format',
        'text',
        '--model',
        'claude-sonnet-5',
        '--no-session-persistence',
        '--system-prompt',
        'you are a compiler',
      ],
      { stdio: ['pipe', 'pipe', 'pipe'] },
    );
    expect(stdinEnd).toHaveBeenCalledWith('compile this');

    child.stdout.emit('data', 'module: greeting\n');
    child.emit('close', 0);

    await expect(promise).resolves.toBe('module: greeting\n');
  });

  it('omits --system-prompt when the system prompt is empty', async () => {
    const { child } = makeFakeChild();
    spawn.mockReturnValue(child);

    const client = createClaudeCliClient('claude-sonnet-5');
    const promise = client.complete({ system: '   ', user: 'hi' });

    const args = spawn.mock.calls[0][1] as string[];
    expect(args).not.toContain('--system-prompt');

    child.stdout.emit('data', 'ok');
    child.emit('close', 0);
    await expect(promise).resolves.toBe('ok');
  });

  it('rejects with a typed error including stderr and a cause on non-zero exit', async () => {
    const { child } = makeFakeChild();
    spawn.mockReturnValue(child);

    const client = createClaudeCliClient('claude-sonnet-5');
    const promise = client.complete({ system: '', user: 'hi' });

    child.stderr.emit('data', 'boom: model unavailable');
    child.emit('close', 7);

    await expect(promise).rejects.toMatchObject({
      name: 'ClaudeCliError',
      stderr: 'boom: model unavailable',
      exitCode: 7,
    });
    await promise.catch((error: { cause?: unknown }) => {
      expect(error.cause).toBeInstanceOf(Error);
    });
  });

  it('rejects with a typed error when the process fails to spawn', async () => {
    const { child } = makeFakeChild();
    spawn.mockReturnValue(child);

    const client = createClaudeCliClient('claude-sonnet-5');
    const promise = client.complete({ system: '', user: 'hi' });

    const cause = new Error('ENOENT');
    child.emit('error', cause);

    await expect(promise).rejects.toMatchObject({ name: 'ClaudeCliError', exitCode: null });
    await promise.catch((error: { cause?: unknown }) => {
      expect(error.cause).toBe(cause);
    });
  });

  it('rejects when the CLI returns empty stdout', async () => {
    const { child } = makeFakeChild();
    spawn.mockReturnValue(child);

    const client = createClaudeCliClient('claude-sonnet-5');
    const promise = client.complete({ system: '', user: 'hi' });

    child.emit('close', 0);
    await expect(promise).rejects.toMatchObject({ name: 'ClaudeCliError' });
  });
});

describe('createLlmClient dispatch', () => {
  it('returns a claude-cli client that spawns claude', async () => {
    const { child } = makeFakeChild();
    spawn.mockReturnValue(child);

    const client = createLlmClient({ backend: 'claude-cli', model: 'claude-sonnet-5' });
    const promise = client.complete({ system: '', user: 'hi' });
    expect(spawn).toHaveBeenCalledOnce();

    child.stdout.emit('data', 'ok');
    child.emit('close', 0);
    await expect(promise).resolves.toBe('ok');
  });

  it('throws when the anthropic-api backend is selected without an api key', () => {
    expect(() => createLlmClient({ backend: 'anthropic-api', model: 'claude-sonnet-5' })).toThrow(/requires an API key/);
  });
});
