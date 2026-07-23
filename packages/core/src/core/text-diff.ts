export interface HunkLine {
  kind: ' ' | '-' | '+';
  text: string;
}

export interface Hunk {
  oldStart: number;
  oldCount: number;
  newStart: number;
  newCount: number;
  lines: HunkLine[];
}

interface DiffOp {
  type: 'equal' | 'del' | 'ins';
  oldLine: number;
  newLine: number;
  text: string;
}

export function toLines(text: string): string[] {
  if (text === '') return [];
  return text.replace(/\r?\n$/, '').split(/\r?\n/);
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

export function unifiedDiff(before: string, after: string, context = 3): string {
  return formatUnified(lcsOps(toLines(before), toLines(after)), context);
}

export function applyUnifiedPatch(before: string, patch: string): string {
  const hunks = parseHunks(patch);
  if (hunks.length === 0) return before;
  const beforeLines = toLines(before);
  const result: string[] = [];
  let oldIdx = 0;
  for (const hunk of hunks) {
    const copyUntil = hunk.oldStart - 1;
    while (oldIdx < copyUntil && oldIdx < beforeLines.length) {
      result.push(beforeLines[oldIdx]);
      oldIdx += 1;
    }
    for (const line of hunk.lines) {
      if (line.kind === ' ') {
        result.push(oldIdx < beforeLines.length ? beforeLines[oldIdx] : line.text);
        oldIdx += 1;
      } else if (line.kind === '-') {
        oldIdx += 1;
      } else {
        result.push(line.text);
      }
    }
  }
  while (oldIdx < beforeLines.length) {
    result.push(beforeLines[oldIdx]);
    oldIdx += 1;
  }
  return result.length === 0 ? '' : `${result.join('\n')}\n`;
}

const HUNK_HEADER = /^@@ -(\d+),(\d+) \+(\d+),(\d+) @@/;

export function parseHunks(diff: string): Hunk[] {
  if (diff.trim() === '') return [];
  const hunks: Hunk[] = [];
  let current: Hunk | null = null;
  for (const raw of diff.split(/\r?\n/)) {
    const header = raw.match(HUNK_HEADER);
    if (header !== null) {
      current = {
        oldStart: Number(header[1]),
        oldCount: Number(header[2]),
        newStart: Number(header[3]),
        newCount: Number(header[4]),
        lines: [],
      };
      hunks.push(current);
      continue;
    }
    if (current === null) continue;
    const marker = raw.charAt(0);
    if (marker === ' ' || marker === '-' || marker === '+') {
      current.lines.push({ kind: marker, text: raw.slice(1) });
    }
  }
  return hunks;
}
