import { githubDarkInit, githubLightInit } from '@uiw/codemirror-theme-github';
import type { Extension } from '@codemirror/state';

export type EditorTheme = 'light' | 'dark';

const sharedSettings = {
  background: 'transparent',
  gutterBackground: 'transparent',
  gutterBorder: 'transparent',
  fontFamily: 'inherit',
} as const;

export const naplDarkTheme: Extension = githubDarkInit({
  settings: {
    ...sharedSettings,
    foreground: 'var(--napl-text)',
    gutterForeground: 'var(--napl-gutter-text)',
    gutterActiveForeground: 'var(--napl-text)',
    caret: 'var(--napl-accent)',
    selection: 'var(--napl-selection)',
    selectionMatch: 'var(--napl-selection)',
    lineHighlight: 'var(--napl-active-line)',
  },
});

export const naplLightTheme: Extension = githubLightInit({
  settings: {
    ...sharedSettings,
    foreground: 'var(--napl-text)',
    gutterForeground: 'var(--napl-gutter-text)',
    gutterActiveForeground: 'var(--napl-text)',
    caret: 'var(--napl-accent)',
    selection: 'var(--napl-selection)',
    selectionMatch: 'var(--napl-selection)',
    lineHighlight: 'var(--napl-active-line)',
  },
});

export const resolveEditorTheme = (theme: EditorTheme): Extension =>
  theme === 'light' ? naplLightTheme : naplDarkTheme;
