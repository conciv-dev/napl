import { readFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import { createHighlighter, type HighlighterGeneric } from 'shiki';
import { describe, expect, it } from 'vitest';
import {
  loadNaplLanguages,
  maplLanguage,
  naplLanguage,
} from '../src/index.ts';

const repoFile = (path: string): string =>
  fileURLToPath(new URL(`../../../${path}`, import.meta.url));

describe('NAPL TextMate grammars', () => {
  it('tokenizes real NAPL frontmatter and Markdown with semantic scopes', async () => {
    const source = await readFile(repoFile('selfhost/body_lines.napl'), 'utf8');
    const highlighter = (await createHighlighter({
      themes: ['github-dark'],
      langs: ['yaml', 'markdown'],
    })) as unknown as HighlighterGeneric<string, string>;

    await loadNaplLanguages(highlighter);

    const tokens = highlighter.codeToTokensBase(source, {
      lang: '🧑',
      theme: 'github-dark',
      includeExplanation: true,
    });
    const selected = tokens
      .flat()
      .filter((token) =>
        ['module', 'given', 'expect', '# Body lines'].includes(token.content),
      )
      .map((token) => ({
        content: token.content,
        color: token.color,
        scopes: token.explanation?.flatMap((item) =>
          item.scopes.map((scope) => scope.scopeName),
        ),
      }));

    expect(selected).toMatchInlineSnapshot(`
      [
        {
          "color": "#85E89D",
          "content": "module",
          "scopes": [
            "source.napl",
            "meta.embedded.block.yaml",
            "entity.name.tag.frontmatter.napl",
          ],
        },
        {
          "color": "#F97583",
          "content": "given",
          "scopes": [
            "source.napl",
            "meta.embedded.block.yaml",
            "keyword.control.test-block.napl",
          ],
        },
        {
          "color": "#F97583",
          "content": "expect",
          "scopes": [
            "source.napl",
            "meta.embedded.block.yaml",
            "keyword.control.test-block.napl",
          ],
        },
        {
          "color": "#F97583",
          "content": "given",
          "scopes": [
            "source.napl",
            "meta.embedded.block.yaml",
            "keyword.control.test-block.napl",
          ],
        },
        {
          "color": "#F97583",
          "content": "expect",
          "scopes": [
            "source.napl",
            "meta.embedded.block.yaml",
            "keyword.control.test-block.napl",
          ],
        },
        {
          "color": "#F97583",
          "content": "given",
          "scopes": [
            "source.napl",
            "meta.embedded.block.yaml",
            "keyword.control.test-block.napl",
          ],
        },
        {
          "color": "#F97583",
          "content": "expect",
          "scopes": [
            "source.napl",
            "meta.embedded.block.yaml",
            "keyword.control.test-block.napl",
          ],
        },
        {
          "color": "#F97583",
          "content": "given",
          "scopes": [
            "source.napl",
            "meta.embedded.block.yaml",
            "keyword.control.test-block.napl",
          ],
        },
        {
          "color": "#F97583",
          "content": "expect",
          "scopes": [
            "source.napl",
            "meta.embedded.block.yaml",
            "keyword.control.test-block.napl",
          ],
        },
        {
          "color": "#F97583",
          "content": "given",
          "scopes": [
            "source.napl",
            "meta.embedded.block.yaml",
            "keyword.control.test-block.napl",
          ],
        },
        {
          "color": "#F97583",
          "content": "expect",
          "scopes": [
            "source.napl",
            "meta.embedded.block.yaml",
            "keyword.control.test-block.napl",
          ],
        },
        {
          "color": "#F97583",
          "content": "given",
          "scopes": [
            "source.napl",
            "meta.embedded.block.yaml",
            "keyword.control.test-block.napl",
          ],
        },
        {
          "color": "#F97583",
          "content": "expect",
          "scopes": [
            "source.napl",
            "meta.embedded.block.yaml",
            "keyword.control.test-block.napl",
          ],
        },
        {
          "color": "#79B8FF",
          "content": "# Body lines",
          "scopes": [
            "source.napl",
            "markup.heading.markdown",
            "heading.1.markdown",
            "punctuation.definition.heading.markdown",
            "source.napl",
            "markup.heading.markdown",
            "heading.1.markdown",
            "source.napl",
            "markup.heading.markdown",
            "heading.1.markdown",
            "entity.name.section.markdown",
          ],
        },
      ]
    `);
    expect(highlighter.getLoadedLanguages()).toContain('🧑');
    highlighter.dispose();
  });

  it('tokenizes real MAPL severity kinds with distinct scopes and colors', async () => {
    const source = await readFile(
      repoFile('selfhost/.napl/mapl/clock_fmt.mapl'),
      'utf8',
    );
    const highlighter = (await createHighlighter({
      themes: ['github-dark'],
      langs: ['yaml'],
    })) as unknown as HighlighterGeneric<string, string>;

    await highlighter.loadLanguage(maplLanguage, naplLanguage);

    const tokens = highlighter.codeToTokensBase(source, {
      lang: '🤖',
      theme: 'github-dark',
      includeExplanation: true,
    });
    const selected = tokens
      .flat()
      .filter((token) => ['ambiguity', 'assumption', 'note'].includes(token.content))
      .map((token) => ({
        content: token.content,
        color: token.color,
        scopes: token.explanation?.flatMap((item) =>
          item.scopes.map((scope) => scope.scopeName),
        ),
      }));

    expect(selected).toMatchInlineSnapshot(`
      [
        {
          "color": "#FDAEB7",
          "content": "ambiguity",
          "scopes": [
            "source.mapl",
            "invalid.illegal.ambiguity.mapl",
          ],
        },
        {
          "color": "#79B8FF",
          "content": "note",
          "scopes": [
            "source.mapl",
            "markup.inline.raw.info.machine-margin.mapl",
          ],
        },
        {
          "color": "#FFAB70",
          "content": "assumption",
          "scopes": [
            "source.mapl",
            "markup.changed.warning.machine-margin.mapl",
          ],
        },
      ]
    `);
    expect(new Set(selected.map((token) => token.color)).size).toBe(3);
    expect(highlighter.getLoadedLanguages()).toContain('🤖');
    highlighter.dispose();
  });
});
