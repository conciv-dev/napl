import type { LanguageInput } from 'shiki';
import maplGrammar from '../mapl.tmLanguage.json' with { type: 'json' };
import naplGrammar from '../napl.tmLanguage.json' with { type: 'json' };

export { maplGrammar, naplGrammar };

export const naplLanguage = {
  ...naplGrammar,
  aliases: ['🧑'],
} satisfies LanguageInput;

export const maplLanguage = {
  ...maplGrammar,
  aliases: ['🤖'],
} satisfies LanguageInput;

export interface LanguageLoader {
  loadLanguage(...languages: LanguageInput[]): Promise<void>;
}

export const loadNaplLanguages = async (
  highlighter: LanguageLoader,
): Promise<void> => {
  await highlighter.loadLanguage(naplLanguage, maplLanguage);
};
