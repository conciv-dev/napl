import { describe, expect, it } from 'vitest';
import { findTargetAtPosition, scanDocument } from '../src/lsp/scanner.js';

const DOC = `---
module: auth/session
deps: [auth/tokens, auth/users]
targets: [typescript]
---
# Session

Manage sessions. See @auth/tokens and @auth/users for details.
Refreshes via @auth/session.
`;

describe('scanDocument', () => {
  it('locates the module value span exactly', () => {
    const scan = scanDocument(DOC);
    expect(scan.moduleValue).not.toBeNull();
    expect(scan.moduleValue?.value).toBe('auth/session');
    expect(scan.moduleValue?.span).toEqual({
      start: { line: 1, character: 8 },
      end: { line: 1, character: 20 },
    });
  });

  it('locates inline deps entries with exact spans', () => {
    const scan = scanDocument(DOC);
    expect(scan.deps.map((d) => d.value)).toEqual(['auth/tokens', 'auth/users']);
    expect(scan.deps.every((d) => d.source === 'deps')).toBe(true);
    const tokens = scan.deps[0];
    expect(tokens.span).toEqual({
      start: { line: 2, character: 7 },
      end: { line: 2, character: 18 },
    });
    const users = scan.deps[1];
    expect(users.span.start).toEqual({ line: 2, character: 20 });
    expect(users.span.end).toEqual({ line: 2, character: 30 });
  });

  it('finds @refs in the body with exact spans and excludes frontmatter', () => {
    const scan = scanDocument(DOC);
    expect(scan.refs.map((r) => r.module)).toEqual(['auth/tokens', 'auth/users', 'auth/session']);
    const first = scan.refs[0];
    expect(first.span.start).toEqual({ line: 7, character: 21 });
    expect(first.span.end).toEqual({ line: 7, character: 33 });
  });

  it('reports frontmatter and body regions', () => {
    const scan = scanDocument(DOC);
    expect(scan.frontmatter.present).toBe(true);
    expect(scan.frontmatter.span?.start).toEqual({ line: 1, character: 0 });
    expect(scan.frontmatter.span?.end.line).toBe(3);
    expect(scan.body.present).toBe(true);
    expect(scan.body.span?.start.line).toBe(5);
  });

  it('handles block-style deps lists', () => {
    const doc = `---
module: a
deps:
  - one
  - "two/x"
extends: base
---
body
`;
    const scan = scanDocument(doc);
    expect(scan.deps.map((d) => `${d.source}:${d.value}`)).toEqual([
      'deps:one',
      'deps:two/x',
      'extends:base',
    ]);
    expect(scan.deps[0].span).toEqual({
      start: { line: 3, character: 4 },
      end: { line: 3, character: 7 },
    });
    expect(scan.deps[1].span.start).toEqual({ line: 4, character: 5 });
    const ext = scan.deps[2];
    expect(ext.span.start).toEqual({ line: 5, character: 9 });
  });

  it('handles empty inline deps', () => {
    const scan = scanDocument(`---\nmodule: greeting\ndeps: []\n---\nbody\n`);
    expect(scan.deps).toEqual([]);
    expect(scan.moduleValue?.value).toBe('greeting');
  });
});

describe('findTargetAtPosition', () => {
  const scan = scanDocument(DOC);

  it('resolves the module value token', () => {
    const target = findTargetAtPosition(scan, { line: 1, character: 10 });
    expect(target).toEqual({
      kind: 'module-value',
      module: 'auth/session',
      span: scan.moduleValue?.span,
    });
  });

  it('resolves a dep token', () => {
    const target = findTargetAtPosition(scan, { line: 2, character: 10 });
    expect(target?.kind).toBe('dep');
    expect(target?.module).toBe('auth/tokens');
  });

  it('resolves an @ref token', () => {
    const target = findTargetAtPosition(scan, { line: 7, character: 25 });
    expect(target?.kind).toBe('ref');
    expect(target?.module).toBe('auth/tokens');
  });

  it('returns null off any token', () => {
    expect(findTargetAtPosition(scan, { line: 5, character: 0 })).toBeNull();
  });
});
