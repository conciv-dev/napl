import type { ActualResult } from './execute.ts';
import type { Failure, FileExpectation, Scenario } from './types.ts';

function subst(text: string, runnerPid: number): string {
  return text.split('{{RUNNER_PID}}').join(String(runnerPid));
}

function toLines(text: string): string[] {
  if (text === '') return [];
  return text.replace(/\n$/, '').split('\n');
}

function matchLine(expected: string, actual: string): boolean {
  if (expected.startsWith('re:')) return new RegExp(expected.slice(3)).test(actual);
  return expected === actual;
}

function compareLines(label: string, expected: string[], actual: string, runnerPid: number, failures: Failure[]): void {
  const actualLines = toLines(actual);
  const expectedLines = expected.map((line) => subst(line, runnerPid));
  if (actualLines.length !== expectedLines.length) {
    failures.push({
      kind: `${label} line count`,
      detail: `expected ${expectedLines.length} line(s), got ${actualLines.length}\n--- expected ---\n${expectedLines.join('\n')}\n--- actual ---\n${actualLines.join('\n')}`,
    });
    return;
  }
  for (let i = 0; i < expectedLines.length; i += 1) {
    if (!matchLine(expectedLines[i], actualLines[i])) {
      failures.push({
        kind: `${label} line ${i + 1}`,
        detail: `expected: ${JSON.stringify(expectedLines[i])}\nactual:   ${JSON.stringify(actualLines[i])}`,
      });
    }
  }
}

function compareContains(label: string, needles: string[], actual: string, runnerPid: number, failures: Failure[]): void {
  for (const needle of needles) {
    const resolved = subst(needle, runnerPid);
    if (!actual.includes(resolved)) {
      failures.push({ kind: `${label} contains`, detail: `missing substring: ${JSON.stringify(resolved)}` });
    }
  }
}

function normalizeMode(mode: string): string {
  return Number.parseInt(mode, 8).toString(8);
}

function compareFile(
  rel: string,
  spec: string | FileExpectation,
  actual: ActualResult,
  runnerPid: number,
  failures: Failure[],
): void {
  const file = actual.files.get(rel);
  if (file === undefined) {
    failures.push({ kind: 'file', detail: `${rel}: not collected` });
    return;
  }
  const expectation: FileExpectation = typeof spec === 'string' ? { content: spec } : spec;

  if (expectation.absent === true) {
    if (file.exists) failures.push({ kind: 'file', detail: `${rel}: expected absent but present` });
    return;
  }
  if (!file.exists) {
    failures.push({ kind: 'file', detail: `${rel}: expected present but missing` });
    return;
  }
  if (expectation.content !== undefined) {
    const expected = subst(expectation.content, runnerPid);
    if (file.content !== expected) {
      failures.push({
        kind: 'file content',
        detail: `${rel} differs\n--- expected ---\n${expected}\n--- actual ---\n${file.content}`,
      });
    }
  }
  for (const needle of expectation.contains ?? []) {
    if (!file.content.includes(subst(needle, runnerPid))) {
      failures.push({ kind: 'file contains', detail: `${rel}: missing ${JSON.stringify(needle)}` });
    }
  }
  if (expectation.matches !== undefined && !new RegExp(expectation.matches).test(file.content)) {
    failures.push({ kind: 'file matches', detail: `${rel}: does not match /${expectation.matches}/` });
  }
  if (expectation.mode !== undefined && normalizeMode(file.mode) !== normalizeMode(expectation.mode)) {
    failures.push({ kind: 'file mode', detail: `${rel}: expected mode ${expectation.mode}, got ${file.mode}` });
  }
}

export function compareScenario(scenario: Scenario, actual: ActualResult): Failure[] {
  const failures: Failure[] = [];
  const runnerPid = process.pid;
  const expect = scenario.expect;

  if (actual.exitCode !== expect.exitCode) {
    failures.push({
      kind: 'exitCode',
      detail: `expected ${expect.exitCode}, got ${actual.exitCode}\n--- stdout ---\n${actual.stdout}\n--- stderr ---\n${actual.stderr}`,
    });
  }
  if (expect.stdout !== undefined) compareLines('stdout', expect.stdout, actual.stdout, runnerPid, failures);
  if (expect.stdoutContains !== undefined) compareContains('stdout', expect.stdoutContains, actual.stdout, runnerPid, failures);
  if (expect.stderr !== undefined) compareLines('stderr', expect.stderr, actual.stderr, runnerPid, failures);
  if (expect.stderrContains !== undefined) compareContains('stderr', expect.stderrContains, actual.stderr, runnerPid, failures);

  for (const [rel, spec] of Object.entries(expect.files ?? {})) {
    compareFile(rel, spec, actual, runnerPid, failures);
  }

  for (const input of expect.agentInputs ?? []) {
    const captured = actual.agentInputs[input.index];
    if (captured === undefined) {
      failures.push({ kind: 'agentInput', detail: `no agent input captured at index ${input.index}` });
      continue;
    }
    for (const needle of input.contains ?? []) {
      if (!captured.includes(subst(needle, runnerPid))) {
        failures.push({ kind: 'agentInput contains', detail: `input ${input.index}: missing ${JSON.stringify(needle)}` });
      }
    }
    if (input.matches !== undefined && !new RegExp(input.matches).test(captured)) {
      failures.push({ kind: 'agentInput matches', detail: `input ${input.index}: does not match /${input.matches}/` });
    }
  }

  return failures;
}
