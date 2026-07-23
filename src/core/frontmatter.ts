import { parse as parseYaml } from 'yaml';
import { z } from 'zod';

export const promptTestSchema = z.object({
  name: z.string(),
  given: z.record(z.string(), z.unknown()).default({}),
  expect: z.record(z.string(), z.unknown()).default({}),
});

export const frontmatterSchema = z.object({
  module: z.string().min(1),
  deps: z.array(z.string()).default([]),
  targets: z.array(z.string()).default([]),
  tests: z.array(promptTestSchema).default([]),
});

export type Frontmatter = z.infer<typeof frontmatterSchema>;

export interface ParsedPrompt {
  frontmatter: Frontmatter;
  body: string;
}

const FRONTMATTER_RE = /^---\r?\n([\s\S]*?)\r?\n---\r?\n?([\s\S]*)$/;

export function parseFrontmatter(raw: string): ParsedPrompt {
  const match = raw.match(FRONTMATTER_RE);
  if (!match) {
    throw new Error('missing YAML frontmatter: a prompt file must start with a --- delimited block');
  }
  const yamlText = match[1];
  const body = match[2];
  let data: unknown;
  try {
    data = parseYaml(yamlText);
  } catch (cause) {
    throw new Error('invalid YAML frontmatter', { cause });
  }
  const parsed = frontmatterSchema.safeParse(data ?? {});
  if (!parsed.success) {
    throw new Error(`invalid frontmatter: ${parsed.error.message}`, { cause: parsed.error });
  }
  return { frontmatter: parsed.data, body: body.replace(/^\s*\n/, '') };
}
