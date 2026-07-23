export interface TestRunCommand {
  command: string;
  args: string[];
}

export interface TargetAdapter {
  name: string;
  idiomGuidance: string;
  agentTools: string[];
  attributionExcludeDirs: string[];
  attributionExcludeFiles: string[];
  attributionExcludeSuffixes: string[];
  testCommandLabel: string;
  testCommand(targetDir: string): TestRunCommand;
}

export const BASE_AGENT_TOOLS = [
  'Read',
  'Edit',
  'Write',
  'Bash(npm:*)',
  'Bash(npx:*)',
  'Bash(node:*)',
  'Bash(mkdir:*)',
  'Bash(ls:*)',
  'Bash(cat:*)',
  'Bash(rm:*)',
  'Bash(mv:*)',
  'Bash(cp:*)',
  'Bash(touch:*)',
  'Bash(echo:*)',
  'Bash(sed:*)',
];

export const BASE_EXCLUDE_DIRS = ['node_modules', 'dist', '.git', 'build', 'coverage', '.vite'];
export const BASE_EXCLUDE_FILES = ['package-lock.json'];
export const BASE_EXCLUDE_SUFFIXES = ['.tsbuildinfo', '.d.ts'];
