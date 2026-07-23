import type { AttributionEntry } from './attribution-schema.js';

export interface BodyLineDiff {
  unified: string;
  changedOldLines: number[];
  changedNewLines: number[];
}

interface DiffOp {
  type: 'equal' | 'del' | 'ins';
  oldLine: number;
  newLine: number;
  text: string;
}

function splitLines(text: string): string[] {
  return text.split(/\r?\n/);
}

function lcsOps(a: string[], b: string[]): DiffOp[] {
  const n = a.length;
  const m = b.length;
  const dp: number[][] = Array.from({ length: n + 1 }, () => new Array<number>(m + 1).fill(0));
  for (let i = n - 1; i >= 0; i -= 1) {
    for (let j = m - 1; j >= 0; j -= 1) {
      dp[i][j] = a[i] === b[j] ? dp[i + 1][j + 1] + 1 : Math.max(dp[i + 1][j], dp[i][j + 1]);
    }
  }
  const ops: DiffOp[] = [];
  let i = 0;
  let j = 0;
  while (i < n && j < m) {
    if (a[i] === b[j]) {
      ops.push({ type: 'equal', oldLine: i + 1, newLine: j + 1, text: a[i] });
      i += 1;
      j += 1;
    } else if (dp[i + 1][j] >= dp[i][j + 1]) {
      ops.push({ type: 'del', oldLine: i + 1, newLine: j + 1, text: a[i] });
      i += 1;
    } else {
      ops.push({ type: 'ins', oldLine: i + 1, newLine: j + 1, text: b[j] });
      j += 1;
    }
  }
  while (i < n) {
    ops.push({ type: 'del', oldLine: i + 1, newLine: j + 1, text: a[i] });
    i += 1;
  }
  while (j < m) {
    ops.push({ type: 'ins', oldLine: i + 1, newLine: j + 1, text: b[j] });
    j += 1;
  }
  return ops;
}

function formatUnified(ops: DiffOp[], context: number): string {
  const include = new Array<boolean>(ops.length).fill(false);
  ops.forEach((op, idx) => {
    if (op.type === 'equal') return;
    const from = Math.max(0, idx - context);
    const to = Math.min(ops.length - 1, idx + context);
    for (let k = from; k <= to; k += 1) include[k] = true;
  });

  const hunks: DiffOp[][] = [];
  let current: DiffOp[] = [];
  for (let idx = 0; idx < ops.length; idx += 1) {
    if (include[idx]) {
      current.push(ops[idx]);
    } else if (current.length > 0) {
      hunks.push(current);
      current = [];
    }
  }
  if (current.length > 0) hunks.push(current);

  const lines: string[] = [];
  for (const hunk of hunks) {
    const oldInHunk = hunk.filter((op) => op.type !== 'ins');
    const newInHunk = hunk.filter((op) => op.type !== 'del');
    const oldStart = oldInHunk.length > 0 ? oldInHunk[0].oldLine : 0;
    const newStart = newInHunk.length > 0 ? newInHunk[0].newLine : 0;
    lines.push(`@@ -${oldStart},${oldInHunk.length} +${newStart},${newInHunk.length} @@`);
    for (const op of hunk) {
      const sign = op.type === 'equal' ? ' ' : op.type === 'del' ? '-' : '+';
      lines.push(`${sign}${op.text}`);
    }
  }
  return lines.join('\n');
}

export function diffBodyLines(oldBody: string, newBody: string, context = 3): BodyLineDiff {
  const ops = lcsOps(splitLines(oldBody), splitLines(newBody));
  const changedOldLines = ops.filter((op) => op.type === 'del').map((op) => op.oldLine);
  const changedNewLines = ops.filter((op) => op.type === 'ins').map((op) => op.newLine);
  return { unified: formatUnified(ops, context), changedOldLines, changedNewLines };
}

export function selectIntersectingEntries(
  entries: readonly AttributionEntry[],
  changedOldLines: readonly number[],
): AttributionEntry[] {
  const changed = new Set(changedOldLines);
  return entries.filter((entry) => {
    for (let line = entry.promptLines[0]; line <= entry.promptLines[1]; line += 1) {
      if (changed.has(line)) return true;
    }
    return false;
  });
}

export function incrementalUnlockList(
  ownedFiles: readonly string[],
  intersectingEntries: readonly AttributionEntry[],
  targetRelToRoot: string,
): string[] {
  const set = new Set(ownedFiles);
  for (const entry of intersectingEntries) {
    set.add(`${targetRelToRoot}/${entry.file}`);
  }
  return [...set].sort();
}
