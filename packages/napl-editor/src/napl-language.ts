import {
  HighlightStyle,
  StreamLanguage,
  syntaxHighlighting,
  type StreamParser,
} from '@codemirror/language';
import type { Extension } from '@codemirror/state';
import { Tag, tags } from '@lezer/highlight';

export interface NaplStreamState {
  section: 'start' | 'frontmatter' | 'body';
}

export const naplFrontmatterKeyTag = Tag.define();
export const naplTestKeyTag = Tag.define();
export const naplRefTag = Tag.define();

const FENCE = /^-{3}[ \t]*$/;
const FRONTMATTER_KEY = /^(module|deps|targets|tests)(?=\s*:)/;
const TEST_KEY = /^(given|expect)(?=\s*:)/;
const GENERIC_KEY = /^[A-Za-z_][\w-]*(?=\s*:)/;
const DOUBLE_STRING = /^"(?:[^"\\]|\\.)*"?/;
const SINGLE_STRING = /^'(?:[^'\\]|\\.)*'?/;
const NUMBER = /^-?\d+(?:\.\d+)?\b/;
const CONSTANT = /^(true|false|null|yes|no|~)\b/;
const SCALAR = /^[^\s#'":[\]{},]+/;

const HEADING = /^#{1,6}\s.*$/;
const LIST_MARKER = /^\s*[-*+]\s+/;
const REF = /^@[A-Za-z0-9_][A-Za-z0-9_./-]*/;
const INLINE_CODE = /^`[^`]*`/;
const BOLD = /^(\*\*|__)[^*_]+(\*\*|__)/;
const ITALIC = /^(\*|_)[^*_\s][^*_]*(\*|_)/;
const WORD = /^[^\s@`*_]+/;

const tokenizeFrontmatter = (
  stream: {
    sol(): boolean;
    match(pattern: RegExp): boolean | RegExpMatchArray | null;
    eatSpace(): boolean;
    eat(match: string): string | void;
    peek(): string | undefined;
    next(): string | void;
    skipToEnd(): void;
  },
  state: NaplStreamState,
): string | null => {
  if (stream.sol() && stream.match(FENCE)) {
    state.section = 'body';
    return 'fence';
  }
  if (stream.eatSpace()) {
    return null;
  }
  if (stream.peek() === '#') {
    stream.skipToEnd();
    return 'comment';
  }
  if (stream.match(/^-(?=\s)/)) {
    return 'punctuation';
  }
  if (stream.match(FRONTMATTER_KEY)) {
    return 'frontmatterKey';
  }
  if (stream.match(TEST_KEY)) {
    return 'testKey';
  }
  if (stream.match(GENERIC_KEY)) {
    return 'yamlKey';
  }
  if (stream.eat(':')) {
    return 'punctuation';
  }
  if (stream.match(DOUBLE_STRING) || stream.match(SINGLE_STRING)) {
    return 'string';
  }
  if (stream.match(/^[[\]{},]/)) {
    return 'punctuation';
  }
  if (stream.match(NUMBER)) {
    return 'number';
  }
  if (stream.match(CONSTANT)) {
    return 'bool';
  }
  if (stream.match(SCALAR)) {
    return null;
  }
  stream.next();
  return null;
};

const tokenizeBody = (
  stream: {
    sol(): boolean;
    match(pattern: RegExp): boolean | RegExpMatchArray | null;
    eatSpace(): boolean;
    next(): string | void;
  },
): string | null => {
  if (stream.sol()) {
    if (stream.match(HEADING)) {
      return 'heading';
    }
    if (stream.match(LIST_MARKER)) {
      return 'punctuation';
    }
  }
  if (stream.match(REF)) {
    return 'ref';
  }
  if (stream.match(INLINE_CODE)) {
    return 'code';
  }
  if (stream.match(BOLD)) {
    return 'strong';
  }
  if (stream.match(ITALIC)) {
    return 'emphasis';
  }
  if (stream.eatSpace()) {
    return null;
  }
  if (stream.match(WORD)) {
    return null;
  }
  stream.next();
  return null;
};

export const naplStreamParser: StreamParser<NaplStreamState> = {
  name: 'napl',
  startState: () => ({ section: 'start' }),
  copyState: (state) => ({ section: state.section }),
  token(stream, state) {
    if (state.section === 'start') {
      if (stream.sol() && stream.match(FENCE)) {
        state.section = 'frontmatter';
        return 'fence';
      }
      state.section = 'body';
    }
    if (state.section === 'frontmatter') {
      return tokenizeFrontmatter(stream, state);
    }
    return tokenizeBody(stream);
  },
  tokenTable: {
    fence: tags.meta,
    frontmatterKey: tags.definition(tags.propertyName),
    testKey: tags.keyword,
    yamlKey: tags.propertyName,
    punctuation: tags.punctuation,
    string: tags.string,
    number: tags.number,
    bool: tags.bool,
    comment: tags.comment,
    heading: tags.heading,
    ref: tags.link,
    code: tags.monospace,
    strong: tags.strong,
    emphasis: tags.emphasis,
  },
  languageData: {
    commentTokens: { line: '#' },
  },
};

export const naplStreamLanguage = StreamLanguage.define(naplStreamParser);

export const naplHighlightStyle = HighlightStyle.define([
  { tag: tags.meta, color: '#6a737d' },
  { tag: naplFrontmatterKeyTag, color: '#85e89d' },
  { tag: naplTestKeyTag, color: '#f97583' },
  { tag: tags.propertyName, color: '#79b8ff' },
  { tag: tags.punctuation, color: '#e1e4e8' },
  { tag: tags.string, color: '#9ecbff' },
  { tag: tags.number, color: '#79b8ff' },
  { tag: tags.bool, color: '#79b8ff' },
  { tag: tags.comment, color: '#6a737d', fontStyle: 'italic' },
  { tag: tags.heading, color: '#79b8ff', fontWeight: 'bold' },
  { tag: naplRefTag, color: '#b392f0', textDecoration: 'underline' },
  { tag: tags.monospace, color: '#85e89d' },
  { tag: tags.strong, fontWeight: 'bold' },
  { tag: tags.emphasis, fontStyle: 'italic' },
]);

export const naplHighlighting: Extension = syntaxHighlighting(naplHighlightStyle);

export const naplLanguage = (): Extension => [naplStreamLanguage, naplHighlighting];
