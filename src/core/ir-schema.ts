import { z } from 'zod';

export const irTypeSchema = z.object({
  name: z.string(),
  description: z.string(),
});

export const irFunctionSchema = z.object({
  name: z.string(),
  signature: z.string(),
  behavior: z.string(),
});

const contractValue = z.union([z.record(z.string(), z.unknown()), z.string()]).default({});

export const irTestSchema = z.object({
  name: z.string(),
  given: contractValue,
  expect: contractValue,
});

export const irSchema = z.object({
  module: z.string().min(1),
  deps: z.array(z.string()).default([]),
  types: z.array(irTypeSchema).default([]),
  functions: z.array(irFunctionSchema).default([]),
  tests: z.array(irTestSchema).default([]),
});

export type Ir = z.infer<typeof irSchema>;

export function validateIr(data: unknown): Ir {
  const parsed = irSchema.safeParse(data);
  if (!parsed.success) {
    throw new Error(`IR validation failed: ${parsed.error.message}`, { cause: parsed.error });
  }
  return parsed.data;
}
