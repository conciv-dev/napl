import type { ScenarioResult } from './types.ts';

const GREEN = '\x1b[32m';
const RED = '\x1b[31m';
const DIM = '\x1b[2m';
const RESET = '\x1b[0m';

export function formatResult(result: ScenarioResult): string {
  const mark = result.passed ? `${GREEN}PASS${RESET}` : `${RED}FAIL${RESET}`;
  const lines = [`${mark}  ${result.name}  ${DIM}(${result.durationMs}ms)${RESET}`];
  if (!result.passed) {
    for (const failure of result.failures) {
      lines.push(`      ${RED}✗${RESET} ${failure.kind}`);
      for (const detailLine of failure.detail.split('\n')) {
        lines.push(`        ${detailLine}`);
      }
    }
  }
  return lines.join('\n');
}

export function formatSummary(results: ScenarioResult[], totalMs: number): string {
  const passed = results.filter((result) => result.passed).length;
  const failed = results.length - passed;
  const color = failed === 0 ? GREEN : RED;
  return `${color}${passed} passed, ${failed} failed${RESET}  (${results.length} scenarios, ${totalMs}ms)`;
}
