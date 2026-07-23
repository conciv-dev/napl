export interface Position {
  line: number;
  character: number;
}

export interface Span {
  start: Position;
  end: Position;
}

export interface RegionSpan {
  present: boolean;
  span: Span | null;
}

export interface FrontmatterKeyToken {
  key: string;
  keySpan: Span;
}

export interface ModuleValueToken {
  value: string;
  span: Span;
}

export type DepSource = 'deps' | 'extends';

export interface DepToken {
  value: string;
  span: Span;
  source: DepSource;
}

export interface RefToken {
  module: string;
  span: Span;
}

export interface ScanResult {
  frontmatter: RegionSpan;
  body: RegionSpan;
  keys: FrontmatterKeyToken[];
  moduleValue: ModuleValueToken | null;
  deps: DepToken[];
  refs: RefToken[];
}

export type Target =
  | { kind: 'module-value'; module: string; span: Span }
  | { kind: 'dep'; module: string; span: Span; source: DepSource }
  | { kind: 'ref'; module: string; span: Span };

const REF_RE = /@[A-Za-z0-9_/-]+/g;
const KEY_RE = /^([A-Za-z0-9_-]+):(.*)$/;
const LIST_ITEM_RE = /^(\s*)-\s+(.*)$/;

function splitLines(text: string): string[] {
  return text.split(/\r?\n/);
}

function pos(line: number, character: number): Position {
  return { line, character };
}

function span(startLine: number, startChar: number, endLine: number, endChar: number): Span {
  return { start: pos(startLine, startChar), end: pos(endLine, endChar) };
}

function stripQuotes(raw: string): { value: string; offset: number } {
  const first = raw[0];
  if ((first === '"' || first === "'") && raw.length >= 2 && raw[raw.length - 1] === first) {
    return { value: raw.slice(1, -1), offset: 1 };
  }
  return { value: raw, offset: 0 };
}

function scanInlineList(
  inner: string,
  baseLine: number,
  baseChar: number,
  source: DepSource,
): DepToken[] {
  const tokens: DepToken[] = [];
  const itemRe = /[^,[\]\s][^,[\]]*/g;
  let match: RegExpExecArray | null = itemRe.exec(inner);
  while (match !== null) {
    const rawItem = match[0].trimEnd();
    if (rawItem !== '') {
      const startInInner = match.index;
      const { value, offset } = stripQuotes(rawItem);
      const startChar = baseChar + startInInner + offset;
      tokens.push({
        value,
        source,
        span: span(baseLine, startChar, baseLine, startChar + value.length),
      });
    }
    match = itemRe.exec(inner);
  }
  return tokens;
}

function scanValueToken(
  rawAfterColon: string,
  line: number,
  keyEndChar: number,
): { text: string; span: Span } | null {
  const leading = rawAfterColon.length - rawAfterColon.trimStart().length;
  const trimmed = rawAfterColon.trim();
  if (trimmed === '') return null;
  const startChar = keyEndChar + leading;
  const { value, offset } = stripQuotes(trimmed);
  const valStart = startChar + offset;
  return { text: value, span: span(line, valStart, line, valStart + value.length) };
}

export function scanDocument(text: string): ScanResult {
  const lines = splitLines(text);
  const keys: FrontmatterKeyToken[] = [];
  const deps: DepToken[] = [];
  const refs: RefToken[] = [];
  let moduleValue: ModuleValueToken | null = null;

  let frontmatter: RegionSpan = { present: false, span: null };
  let body: RegionSpan = { present: true, span: null };

  const hasFrontmatter = lines.length > 0 && lines[0].trimEnd() === '---';
  let closeLine = -1;
  if (hasFrontmatter) {
    for (let i = 1; i < lines.length; i += 1) {
      if (lines[i].trimEnd() === '---') {
        closeLine = i;
        break;
      }
    }
  }

  if (hasFrontmatter && closeLine !== -1) {
    const innerStart = 1;
    const innerEnd = closeLine - 1;
    frontmatter = {
      present: true,
      span:
        innerEnd >= innerStart
          ? span(innerStart, 0, innerEnd, lines[innerEnd].length)
          : span(innerStart, 0, innerStart, 0),
    };

    let currentListKey: DepSource | null = null;
    for (let i = innerStart; i <= innerEnd; i += 1) {
      const raw = lines[i];
      const listMatch = raw.match(LIST_ITEM_RE);
      if (listMatch !== null && currentListKey !== null) {
        const indent = listMatch[1].length;
        const dashAndSpace = raw.slice(indent).match(/^-\s+/);
        const valueStartChar = indent + (dashAndSpace ? dashAndSpace[0].length : 2);
        const rawItem = listMatch[2].trim();
        if (rawItem !== '') {
          const { value, offset } = stripQuotes(rawItem);
          const startChar = valueStartChar + offset;
          deps.push({
            value,
            source: currentListKey,
            span: span(i, startChar, i, startChar + value.length),
          });
        }
        continue;
      }

      const keyMatch = raw.match(KEY_RE);
      if (keyMatch === null) {
        continue;
      }
      currentListKey = null;
      const key = keyMatch[1];
      const afterColon = keyMatch[2];
      const keyEndChar = key.length + 1;
      keys.push({ key, keySpan: span(i, 0, i, key.length) });

      if (key === 'module') {
        const valueToken = scanValueToken(afterColon, i, keyEndChar);
        if (valueToken !== null) {
          moduleValue = { value: valueToken.text, span: valueToken.span };
        }
        continue;
      }

      if (key === 'deps' || key === 'extends') {
        const source: DepSource = key;
        const trimmedAfter = afterColon.trim();
        if (trimmedAfter.startsWith('[')) {
          const openIdx = afterColon.indexOf('[');
          const closeIdx = afterColon.lastIndexOf(']');
          const inner = closeIdx > openIdx ? afterColon.slice(openIdx + 1, closeIdx) : afterColon.slice(openIdx + 1);
          deps.push(...scanInlineList(inner, i, keyEndChar + openIdx + 1, source));
        } else if (trimmedAfter === '') {
          currentListKey = source;
        } else {
          const valueToken = scanValueToken(afterColon, i, keyEndChar);
          if (valueToken !== null) {
            deps.push({ value: valueToken.text, source, span: valueToken.span });
          }
        }
      }
    }

    const bodyStartLine = closeLine + 1;
    if (bodyStartLine < lines.length) {
      body = {
        present: true,
        span: span(bodyStartLine, 0, lines.length - 1, lines[lines.length - 1].length),
      };
    } else {
      body = { present: false, span: null };
    }
  } else {
    body = {
      present: lines.length > 0,
      span: lines.length > 0 ? span(0, 0, lines.length - 1, lines[lines.length - 1].length) : null,
    };
  }

  const bodyStart = hasFrontmatter && closeLine !== -1 ? closeLine + 1 : 0;
  for (let i = bodyStart; i < lines.length; i += 1) {
    const line = lines[i];
    REF_RE.lastIndex = 0;
    let m: RegExpExecArray | null = REF_RE.exec(line);
    while (m !== null) {
      const startChar = m.index;
      const full = m[0];
      refs.push({
        module: full.slice(1),
        span: span(i, startChar, i, startChar + full.length),
      });
      m = REF_RE.exec(line);
    }
  }

  return { frontmatter, body, keys, moduleValue, deps, refs };
}

function withinSpan(target: Span, position: Position): boolean {
  const { start, end } = target;
  if (position.line < start.line || position.line > end.line) return false;
  if (position.line === start.line && position.character < start.character) return false;
  if (position.line === end.line && position.character > end.character) return false;
  return true;
}

export function findTargetAtPosition(scan: ScanResult, position: Position): Target | null {
  if (scan.moduleValue !== null && withinSpan(scan.moduleValue.span, position)) {
    return { kind: 'module-value', module: scan.moduleValue.value, span: scan.moduleValue.span };
  }
  for (const dep of scan.deps) {
    if (withinSpan(dep.span, position)) {
      return { kind: 'dep', module: dep.value, span: dep.span, source: dep.source };
    }
  }
  for (const ref of scan.refs) {
    if (withinSpan(ref.span, position)) {
      return { kind: 'ref', module: ref.module, span: ref.span };
    }
  }
  return null;
}
