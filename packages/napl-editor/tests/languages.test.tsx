import { cleanup, render } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { NaplEditor } from '../src/NaplEditor.tsx';
import { languageForFilename, resolveLanguage } from '../src/languages.ts';

afterEach(() => {
  cleanup();
});

describe('languageForFilename', () => {
  it('maps real generated filenames to editor languages', () => {
    expect(languageForFilename('gen_prompt_diff/src/lib.rs')).toBe('rust');
    expect(languageForFilename('src/greeting.ts')).toBe('typescript');
    expect(languageForFilename('package.json')).toBe('json');
    expect(languageForFilename('Cargo.toml')).toBe('toml');
    expect(languageForFilename('greeting.napl')).toBe('napl');
    expect(languageForFilename('greeting.mapl')).toBe('mapl');
    expect(languageForFilename('README')).toBe('text');
  });

  it('resolves each language to a codemirror extension', () => {
    for (const language of ['napl', 'typescript', 'rust', 'json', 'yaml', 'toml'] as const) {
      expect(resolveLanguage(language)).toBeTruthy();
    }
  });
});

describe('NaplEditor language modes', () => {
  it('tokenizes a typescript file into highlighted spans', () => {
    const { container } = render(
      <NaplEditor
        value={'export const greet = (name: string): string => `Hello, ${name}!`;\n'}
        language="typescript"
        readOnly
      />,
    );
    const content = container.querySelector('.cm-content');
    expect(content?.textContent).toContain('greet');
    const tokens = container.querySelectorAll('.cm-content .cm-line span');
    expect(tokens.length).toBeGreaterThan(0);
  });

  it('tokenizes generated rust into highlighted spans', () => {
    const { container } = render(
      <NaplEditor
        value={'pub fn compute() -> String {\n  String::new()\n}\n'}
        language="rust"
        readOnly
      />,
    );
    const tokens = container.querySelectorAll('.cm-content .cm-line span');
    expect(tokens.length).toBeGreaterThan(0);
  });
});
