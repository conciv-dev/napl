import type { AttributionEntry } from '../core/attribution-schema.js';

const GENERATED_PREFIX = '.hl/src/';

export const DRIFT_LENS_PREFIX = 'DRIFT — edits here are not reflected in any prompt';

export interface GeneratedPathInfo {
  target: string;
  targetRelPath: string;
}

export function parseGeneratedPath(relFull: string): GeneratedPathInfo | null {
  const normalized = relFull.split('\\').join('/');
  if (!normalized.startsWith(GENERATED_PREFIX)) return null;
  const rest = normalized.slice(GENERATED_PREFIX.length);
  const slash = rest.indexOf('/');
  if (slash <= 0) return null;
  const target = rest.slice(0, slash);
  const targetRelPath = rest.slice(slash + 1);
  if (targetRelPath.length === 0) return null;
  return { target, targetRelPath };
}

export interface AttributionSource {
  module: string;
  target: string;
  entries: AttributionEntry[];
  promptFiles: string[];
}

export interface ReverseMatch {
  module: string;
  target: string;
  promptFile: string;
  note: string;
  promptLines: [number, number];
  codeLines: [number, number];
}

function inRange(value: number, range: readonly [number, number]): boolean {
  return value >= range[0] && value <= range[1];
}

export function reverseMatches(
  sources: readonly AttributionSource[],
  target: string,
  targetRelPath: string,
  codeLine: number | null,
): ReverseMatch[] {
  const matches: ReverseMatch[] = [];
  for (const source of sources) {
    if (source.target !== target) continue;
    for (const entry of source.entries) {
      if (entry.file !== targetRelPath) continue;
      if (codeLine !== null && !inRange(codeLine, entry.lines)) continue;
      for (const promptFile of source.promptFiles) {
        matches.push({
          module: source.module,
          target: source.target,
          promptFile,
          note: entry.note,
          promptLines: [entry.promptLines[0], entry.promptLines[1]],
          codeLines: [entry.lines[0], entry.lines[1]],
        });
      }
    }
  }
  return matches;
}

export function promptAbsoluteLines(
  bodyStartLine: number,
  promptLines: readonly [number, number],
): [number, number] {
  return [bodyStartLine + promptLines[0] - 1, bodyStartLine + promptLines[1] - 1];
}

export function isFileDrifted(recordedHash: string | undefined, actualHash: string): boolean {
  return recordedHash !== undefined && recordedHash !== actualHash;
}

export function codeLensTitle(promptBasename: string, absoluteLine: number, note: string): string {
  const suffix = note.length > 0 ? ` — ${note}` : '';
  return `⇠ ${promptBasename}:${absoluteLine}${suffix}`;
}

export function dedupeMatches(matches: readonly ReverseMatch[]): ReverseMatch[] {
  const seen = new Set<string>();
  const result: ReverseMatch[] = [];
  for (const match of matches) {
    const key = `${match.promptFile}#${match.promptLines[0]}:${match.promptLines[1]}`;
    if (seen.has(key)) continue;
    seen.add(key);
    result.push(match);
  }
  return result;
}
