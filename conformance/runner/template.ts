export interface TemplateVars {
  cwd: string;
  runnerPid: number;
}

export function applyTemplate(text: string, vars: TemplateVars): string {
  return text
    .split('{{CWD}}')
    .join(vars.cwd)
    .split('{{RUNNER_PID}}')
    .join(String(vars.runnerPid));
}

export function normalizePaths(text: string, cwd: string, realCwd: string): string {
  let out = text;
  if (realCwd !== cwd) out = out.split(realCwd).join('{{CWD}}');
  out = out.split(cwd).join('{{CWD}}');
  return out;
}
