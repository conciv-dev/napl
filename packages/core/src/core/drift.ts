import { existsSync } from 'node:fs';
import { readFile } from 'node:fs/promises';
import { join } from 'node:path';
import { contentHash } from './hash.js';
import type { JournalEntry } from './journal.js';
import { reconstructFileContent } from './journal.js';
import type { NaplMap } from './map.js';
import { unifiedDiff } from './text-diff.js';

export type DriftReason = 'edited' | 'missing';

export interface DriftedFile {
  file: string;
  reason: DriftReason;
  expectedHash?: string;
  actualHash?: string;
  baseline: string | null;
  current: string | null;
  diff: string | null;
}

export interface ModuleDrift {
  module: string;
  promptFile: string;
  target: string;
  files: DriftedFile[];
}

export interface DetectGenDriftInput {
  root: string;
  target: string;
  map: NaplMap;
  journal: readonly JournalEntry[];
  moduleScope?: string;
}

async function classifyFile(
  root: string,
  target: string,
  filePath: string,
  map: NaplMap,
  journal: readonly JournalEntry[],
): Promise<DriftedFile | null> {
  const abs = join(root, filePath);
  const expectedHash = map.files[filePath]?.hash;
  const baseline = reconstructFileContent(journal, filePath);
  if (!existsSync(abs)) {
    return { file: filePath, reason: 'missing', expectedHash, actualHash: undefined, baseline, current: null, diff: null };
  }
  const current = await readFile(abs, 'utf8');
  const actualHash = contentHash(current);
  if (expectedHash !== undefined && actualHash === expectedHash) return null;
  const diff = baseline !== null ? unifiedDiff(baseline, current) : null;
  return { file: filePath, reason: 'edited', expectedHash, actualHash, baseline, current, diff };
}

export async function detectGenDrift(input: DetectGenDriftInput): Promise<ModuleDrift[]> {
  const { root, target, map, journal, moduleScope } = input;
  const drifts: ModuleDrift[] = [];
  for (const [promptFile, record] of Object.entries(map.prompts)) {
    if (moduleScope !== undefined && record.module !== moduleScope) continue;
    const targetRecord = record.targets[target];
    if (targetRecord === undefined || targetRecord.unattributed === true) continue;
    const files: DriftedFile[] = [];
    for (const filePath of targetRecord.files) {
      const drifted = await classifyFile(root, target, filePath, map, journal);
      if (drifted !== null) files.push(drifted);
    }
    if (files.length > 0) drifts.push({ module: record.module, promptFile, target, files });
  }
  return drifts;
}

function indent(text: string, pad: string): string {
  return text
    .split('\n')
    .map((line) => `${pad}${line}`)
    .join('\n');
}

function formatFile(file: DriftedFile): string {
  const lines: string[] = [];
  if (file.reason === 'missing') {
    lines.push(`    ${file.file} (missing — the locked file was deleted)`);
    if (file.expectedHash !== undefined) lines.push(`      recorded hash: ${file.expectedHash}`);
    return lines.join('\n');
  }
  lines.push(`    ${file.file} (edited by hand)`);
  if (file.diff !== null && file.diff.trim() !== '') {
    lines.push('      recorded baseline -> current:');
    lines.push(indent(file.diff, '      '));
  } else {
    lines.push('      baseline content is not recoverable from the journal (pre-journal state); comparing hashes only:');
    lines.push(`      recorded: ${file.expectedHash ?? '(none)'}`);
    lines.push(`      current:  ${file.actualHash ?? '(none)'}`);
  }
  return lines.join('\n');
}

export function formatGenDriftReport(drifts: readonly ModuleDrift[], target: string): string {
  const lines: string[] = [];
  lines.push(
    `BLOCKED  drift detected — cannot run 'napl gen ${target}' while generated files have hand edits that are not reflected in any prompt.`,
  );
  lines.push('');
  for (const drift of drifts) {
    const count = drift.files.length;
    lines.push(`  module ${drift.module} (${drift.target}) — ${count} file(s) drifted (from ${drift.promptFile}):`);
    for (const file of drift.files) lines.push(formatFile(file));
    lines.push('');
    lines.push('  Resolve it one of three ways:');
    lines.push(`    1) napl reconcile ${drift.module}  — fold this edit back into your prompt (coming soon)`);
    lines.push(`    2) napl gen ${target} --module ${drift.module} --force  — discard the edit, the prompt wins`);
    lines.push(`    3) edit the prompt to describe the change, then napl gen ${target}`);
    lines.push('');
  }
  return lines.join('\n');
}
