import { useState, type ReactElement, type ReactNode } from 'react';
import { NaplEditor, type HighlightRange } from './NaplEditor.tsx';
import type { DiagnosticsSource, HoverSource } from './editor-extensions.ts';
import { languageForFilename, type EditorLanguage } from './languages.ts';
import type { EditorTheme } from './editor-theme.ts';

export type PlaygroundLanguage = 'napl' | 'source' | 'mapl';

export interface PlaygroundFile {
  name: string;
  language?: PlaygroundLanguage;
  content: string;
  readOnly?: boolean;
}

export interface PlaygroundShellProps {
  files: PlaygroundFile[];
  activeFile?: string;
  onActiveFileChange?: (name: string) => void;
  onFileChange?: (name: string, content: string) => void;
  diagnostics?: DiagnosticsSource;
  hover?: HoverSource;
  highlightRanges?: HighlightRange[];
  theme?: EditorTheme;
  output?: ReactNode;
  title?: string;
}

const editorLanguageForFile = (file: PlaygroundFile): EditorLanguage => {
  if (file.language === 'napl') return 'napl';
  if (file.language === 'mapl') return 'mapl';
  return languageForFilename(file.name);
};

const languageLabel = (language: PlaygroundLanguage | undefined): string => {
  switch (language) {
    case 'mapl':
      return 'machine';
    case 'source':
      return 'generated';
    default:
      return 'prompt';
  }
};

export const PlaygroundShell = ({
  files,
  activeFile,
  onActiveFileChange,
  onFileChange,
  diagnostics,
  hover,
  highlightRanges,
  theme = 'dark',
  output,
  title,
}: PlaygroundShellProps): ReactElement => {
  const fallback = files[0]?.name ?? '';
  const [internalActive, setInternalActive] = useState(fallback);
  const active = activeFile ?? internalActive;
  const current = files.find((file) => file.name === active) ?? files[0];

  const selectFile = (name: string): void => {
    if (activeFile === undefined) {
      setInternalActive(name);
    }
    onActiveFileChange?.(name);
  };

  const isPrompt = (current?.language ?? 'napl') === 'napl';

  return (
    <div className="napl-playground dark" data-testid="napl-playground">
      <div className="napl-playground__chrome">
        <div className="napl-playground__dots" aria-hidden="true">
          <span />
          <span />
          <span />
        </div>
        {title ? <span className="napl-playground__title">{title}</span> : null}
      </div>
      <div className="napl-playground__tabs" role="tablist">
        {files.map((file) => (
          <button
            type="button"
            key={file.name}
            role="tab"
            aria-selected={file.name === current?.name}
            data-language={file.language ?? 'napl'}
            className={
              file.name === current?.name
                ? 'napl-playground__tab napl-playground__tab--active'
                : 'napl-playground__tab'
            }
            onClick={() => selectFile(file.name)}
          >
            <span className="napl-playground__tab-kind">{languageLabel(file.language)}</span>
            <span className="napl-playground__tab-name">{file.name}</span>
          </button>
        ))}
      </div>
      <div className="napl-playground__body">
        <div className="napl-playground__editor">
          {current ? (
            <NaplEditor
              key={current.name}
              value={current.content}
              readOnly={current.readOnly ?? !isPrompt}
              language={editorLanguageForFile(current)}
              theme={theme}
              diagnostics={isPrompt ? diagnostics : undefined}
              hover={hover}
              highlightRanges={highlightRanges}
              onChange={(next) => onFileChange?.(current.name, next)}
            />
          ) : null}
        </div>
        <div className="napl-playground__margin" data-testid="napl-playground-output">
          {output ?? <div className="napl-playground__placeholder">Output appears here.</div>}
        </div>
      </div>
    </div>
  );
};
