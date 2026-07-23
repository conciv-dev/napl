import { describe, expect, it } from 'vitest';
import { parseFrontmatter } from '../src/core/frontmatter.js';

const VALID = `---
module: auth/session
deps: [auth/tokens]
targets: [typescript]
tests:
  - name: expired token rejected
    given: { token: expired }
    expect: { error: SESSION_EXPIRED }
---
Manage user sessions. Sessions expire after 30 minutes.
`;

describe('parseFrontmatter', () => {
  it('parses frontmatter and body', () => {
    const { frontmatter, body } = parseFrontmatter(VALID);
    expect(frontmatter.module).toBe('auth/session');
    expect(frontmatter.deps).toEqual(['auth/tokens']);
    expect(frontmatter.targets).toEqual(['typescript']);
    expect(frontmatter.tests).toHaveLength(1);
    expect(frontmatter.tests[0]).toEqual({
      name: 'expired token rejected',
      given: { token: 'expired' },
      expect: { error: 'SESSION_EXPIRED' },
    });
    expect(body.startsWith('Manage user sessions.')).toBe(true);
  });

  it('applies defaults for optional fields', () => {
    const { frontmatter } = parseFrontmatter('---\nmodule: solo\n---\nBody here.\n');
    expect(frontmatter.deps).toEqual([]);
    expect(frontmatter.targets).toEqual([]);
    expect(frontmatter.tests).toEqual([]);
  });

  it('throws when frontmatter is missing', () => {
    expect(() => parseFrontmatter('no frontmatter here')).toThrow(/frontmatter/);
  });

  it('throws when module is absent', () => {
    expect(() => parseFrontmatter('---\ndeps: []\n---\nbody')).toThrow(/invalid frontmatter/);
  });
});
