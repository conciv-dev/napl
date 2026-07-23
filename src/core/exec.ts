import { spawn } from 'node:child_process';

export interface CommandResult {
  code: number;
  output: string;
}

export function runCommand(command: string, args: string[], cwd: string): Promise<CommandResult> {
  return new Promise((resolve) => {
    const child = spawn(command, args, { cwd, shell: false });
    let output = '';
    child.stdout.on('data', (chunk: Buffer) => {
      output += chunk.toString();
    });
    child.stderr.on('data', (chunk: Buffer) => {
      output += chunk.toString();
    });
    child.on('error', (err) => {
      resolve({ code: 1, output: `${output}\n${err.message}` });
    });
    child.on('close', (code) => {
      resolve({ code: code ?? 1, output });
    });
  });
}
