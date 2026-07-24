import { StreamLanguage } from '@codemirror/language';
import { javascript } from '@codemirror/lang-javascript';
import { json } from '@codemirror/lang-json';
import { yaml } from '@codemirror/lang-yaml';
import { rust } from '@codemirror/legacy-modes/mode/rust';
import { toml } from '@codemirror/legacy-modes/mode/toml';
import type { Extension } from '@codemirror/state';
import { maplLanguage, naplStreamLanguage } from './napl-language.ts';

export type EditorLanguage =
  | 'napl'
  | 'mapl'
  | 'javascript'
  | 'typescript'
  | 'json'
  | 'yaml'
  | 'toml'
  | 'rust'
  | 'text';

export const resolveLanguage = (language: EditorLanguage): Extension => {
  switch (language) {
    case 'napl':
      return naplStreamLanguage;
    case 'mapl':
      return maplLanguage();
    case 'yaml':
      return yaml();
    case 'javascript':
      return javascript();
    case 'typescript':
      return javascript({ typescript: true });
    case 'json':
      return json();
    case 'toml':
      return StreamLanguage.define(toml);
    case 'rust':
      return StreamLanguage.define(rust);
    default:
      return [];
  }
};

export const languageForFilename = (name: string): EditorLanguage => {
  const lower = name.toLowerCase();
  if (name.endsWith('.🧑') || lower.endsWith('.napl')) return 'napl';
  if (name.endsWith('.🤖') || lower.endsWith('.mapl')) return 'mapl';
  if (lower.endsWith('.tsx') || lower.endsWith('.ts')) return 'typescript';
  if (
    lower.endsWith('.jsx') ||
    lower.endsWith('.js') ||
    lower.endsWith('.mjs') ||
    lower.endsWith('.cjs')
  )
    return 'javascript';
  if (lower.endsWith('.json')) return 'json';
  if (lower.endsWith('.yaml') || lower.endsWith('.yml')) return 'yaml';
  if (lower.endsWith('.toml')) return 'toml';
  if (lower.endsWith('.rs')) return 'rust';
  return 'text';
};
