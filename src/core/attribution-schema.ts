import { z } from 'zod';

export const lineRangeSchema = z.preprocess((value) => {
  if (typeof value === 'number') return [value, value];
  if (Array.isArray(value) && value.length === 1) return [value[0], value[0]];
  return value;
}, z.tuple([z.number().int().min(1), z.number().int().min(1)]));

export const attributionEntrySchema = z.object({
  promptLines: lineRangeSchema,
  file: z.string().min(1),
  lines: lineRangeSchema,
  note: z.string().default(''),
});

export const attributionSchema = z.object({
  module: z.string().min(1),
  target: z.string().min(1),
  entries: z.array(attributionEntrySchema).default([]),
});

export type LineRange = z.infer<typeof lineRangeSchema>;
export type AttributionEntry = z.infer<typeof attributionEntrySchema>;
export type Attribution = z.infer<typeof attributionSchema>;

export function validateAttribution(data: unknown): Attribution {
  const parsed = attributionSchema.safeParse(data);
  if (!parsed.success) {
    throw new Error(`attribution validation failed: ${parsed.error.message}`, { cause: parsed.error });
  }
  return parsed.data;
}

export function parseAttributionEntries(data: unknown): AttributionEntry[] {
  const list = Array.isArray(data) ? data : [];
  const parsed = z.array(attributionEntrySchema).safeParse(list);
  if (!parsed.success) {
    throw new Error(`attribution entries invalid: ${parsed.error.message}`, { cause: parsed.error });
  }
  return parsed.data;
}

export function entriesAtBodyLine(attribution: Attribution, bodyLine: number): AttributionEntry[] {
  return attribution.entries.filter(
    (entry) => bodyLine >= entry.promptLines[0] && bodyLine <= entry.promptLines[1],
  );
}
