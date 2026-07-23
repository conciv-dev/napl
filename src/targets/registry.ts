import { reactAdapter } from './react.js';
import { typescriptAdapter } from './typescript.js';
import type { TargetAdapter } from './types.js';

const ADAPTERS: Record<string, TargetAdapter> = {
  [typescriptAdapter.name]: typescriptAdapter,
  [reactAdapter.name]: reactAdapter,
};

export function listTargets(): string[] {
  return Object.keys(ADAPTERS);
}

export function getAdapter(name: string): TargetAdapter {
  const adapter = ADAPTERS[name];
  if (!adapter) {
    throw new Error(`unknown target '${name}'. Available targets: ${listTargets().join(', ')}`);
  }
  return adapter;
}
