export interface AgentStep {
  files?: Record<string, string | { content?: string; delete?: boolean } | null>;
  output?: string;
  code?: number;
}

export interface FakeScript {
  agent?: AgentStep[];
  ir?: string[];
  attribution?: string[];
  ml?: string[];
}

export interface FileExpectation {
  content?: string;
  absent?: boolean;
  mode?: string;
  contains?: string[];
  matches?: string;
}

export interface AgentInputExpectation {
  index: number;
  contains?: string[];
  matches?: string;
}

export interface ScenarioExpect {
  exitCode: number;
  stdout?: string[];
  stdoutContains?: string[];
  stderr?: string[];
  stderrContains?: string[];
  files?: Record<string, string | FileExpectation>;
  agentInputs?: AgentInputExpectation[];
}

export interface Scenario {
  name: string;
  description: string;
  setup?: Record<string, string>;
  env?: Record<string, string>;
  run: string[];
  testExit?: number;
  testOutput?: string;
  script?: FakeScript;
  expect: ScenarioExpect;
}

export interface Failure {
  kind: string;
  detail: string;
}

export interface ScenarioResult {
  name: string;
  description: string;
  passed: boolean;
  failures: Failure[];
  durationMs: number;
}
