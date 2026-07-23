import { describe, expect, it } from 'vitest';
import { validateIr } from '../src/core/ir-schema.js';

describe('validateIr', () => {
  it('accepts a well-formed IR and applies defaults', () => {
    const ir = validateIr({
      module: 'greeting',
      functions: [{ name: 'greet', signature: 'greet(name): string', behavior: 'returns Hello' }],
    });
    expect(ir.module).toBe('greeting');
    expect(ir.deps).toEqual([]);
    expect(ir.types).toEqual([]);
    expect(ir.tests).toEqual([]);
    expect(ir.functions[0].name).toBe('greet');
  });

  it('rejects IR without a module name', () => {
    expect(() => validateIr({ functions: [] })).toThrow(/IR validation failed/);
  });

  it('rejects malformed function entries', () => {
    expect(() => validateIr({ module: 'x', functions: [{ name: 'f' }] })).toThrow(
      /IR validation failed/,
    );
  });
});
