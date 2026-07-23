import { existsSync } from 'node:fs';
import { appendFile, mkdir, readFile } from 'node:fs/promises';
import { dirname } from 'node:path';
import { z } from 'zod';
import type { BlameSourceEntry } from './blame.js';
import { applyUnifiedPatch, unifiedDiff } from './text-diff.js';

export const journalFileSchema = z.object({
  path: z.string().min(1),
  patch: z.string(),
  hashBefore: z.string().nullable(),
  hashAfter: z.string(),
});

export const journalEntrySchema = z.object({
  gen: z.number().int().min(1),
  timestamp: z.string(),
  module: z.string().min(1),
  target: z.string().min(1),
  promptHash: z.string(),
  promptDiff: z.string(),
  mode: z.enum(['full', 'incremental']),
  files: z.array(journalFileSchema).default([]),
});

export type JournalFile = z.infer<typeof journalFileSchema>;
export type JournalEntry = z.infer<typeof journalEntrySchema>;

export function filePatch(before: string | null, after: string): string {
  return unifiedDiff(before ?? '', after);
}

export async function readJournal(journalPath: string, warn?: (message: string) => void): Promise<JournalEntry[]> {
  if (!existsSync(journalPath)) return [];
  const raw = await readFile(journalPath, 'utf8');
  const entries: JournalEntry[] = [];
  const lines = raw.split(/\r?\n/);
  for (let index = 0; index < lines.length; index += 1) {
    const line = lines[index];
    if (line.trim() === '') continue;
    let data: unknown;
    try {
      data = JSON.parse(line);
    } catch {
      warn?.(`journal: skipping corrupt line ${index + 1} (invalid JSON)`);
      continue;
    }
    const parsed = journalEntrySchema.safeParse(data);
    if (!parsed.success) {
      warn?.(`journal: skipping corrupt line ${index + 1} (${parsed.error.message})`);
      continue;
    }
    entries.push(parsed.data);
  }
  return entries;
}

export async function appendJournalEntry(journalPath: string, entry: JournalEntry): Promise<void> {
  await mkdir(dirname(journalPath), { recursive: true });
  await appendFile(journalPath, `${JSON.stringify(entry)}\n`, 'utf8');
}

export function nextGenNumber(entries: readonly JournalEntry[]): number {
  let max = 0;
  for (const entry of entries) {
    if (entry.gen > max) max = entry.gen;
  }
  return max + 1;
}

export function reconstructFileContent(entries: readonly JournalEntry[], filePath: string): string | null {
  const ordered = [...entries].sort((a, b) => a.gen - b.gen);
  let content: string | null = null;
  for (const entry of ordered) {
    const file = entry.files.find((candidate) => candidate.path === filePath);
    if (file === undefined) continue;
    content = applyUnifiedPatch(content ?? '', file.patch);
  }
  return content;
}

export function fileHistory(entries: readonly JournalEntry[], filePath: string): BlameSourceEntry[] {
  const history: BlameSourceEntry[] = [];
  for (const entry of entries) {
    const file = entry.files.find((candidate) => candidate.path === filePath);
    if (file === undefined) continue;
    history.push({
      gen: entry.gen,
      timestamp: entry.timestamp,
      module: entry.module,
      patch: file.patch,
      promptDiff: entry.promptDiff,
    });
  }
  return history;
}
