import { EditorState, type Extension } from '@codemirror/state';
import { linter, type Diagnostic } from '@codemirror/lint';
import { EditorView, hoverTooltip, tooltips } from '@codemirror/view';
import { resolveLanguage, type EditorLanguage } from './languages.ts';
import { resolveEditorTheme, type EditorTheme } from './editor-theme.ts';

export type EditorSeverity = 'error' | 'warning' | 'info';

export interface EditorDiagnostic {
  line: number;
  endLine?: number;
  column?: number;
  endColumn?: number;
  severity: EditorSeverity;
  message: string;
}

export type DiagnosticsSource = (
  content: string,
) => EditorDiagnostic[] | Promise<EditorDiagnostic[]>;

export interface HoverContext {
  line: number;
  column: number;
  content: string;
}

export interface HoverCardExcerpt {
  code: string;
  caption?: string;
  language?: EditorLanguage;
  theme?: EditorTheme;
}

export interface HoverCardJump {
  label: string;
  onJump: () => void;
}

export interface HoverCard {
  kind: 'card';
  heading?: string;
  quote?: string;
  excerpt?: HoverCardExcerpt;
  meta?: string;
  jump?: HoverCardJump;
}

export type HoverResult = string | HoverCard | null;

export type HoverSource = (
  context: HoverContext,
) => HoverResult | Promise<HoverResult>;

const toCmDiagnostic = (view: EditorView, diagnostic: EditorDiagnostic): Diagnostic => {
  const { doc } = view.state;
  const startLineNo = Math.min(Math.max(diagnostic.line + 1, 1), doc.lines);
  const endLineNo = Math.min(
    Math.max((diagnostic.endLine ?? diagnostic.line) + 1, 1),
    doc.lines,
  );
  const startLine = doc.line(startLineNo);
  const endLine = doc.line(endLineNo);
  const from =
    diagnostic.column != null
      ? Math.min(startLine.from + diagnostic.column, startLine.to)
      : startLine.from;
  const to =
    diagnostic.endColumn != null
      ? Math.min(endLine.from + diagnostic.endColumn, endLine.to)
      : endLine.to;
  return {
    from,
    to: Math.max(to, from),
    severity: diagnostic.severity,
    message: diagnostic.message,
  };
};

export const naplLinter = (source: DiagnosticsSource): Extension =>
  linter(async (view) => {
    const items = await source(view.state.doc.toString());
    return items.map((item) => toCmDiagnostic(view, item));
  });

const mountExcerpt = (
  container: HTMLElement,
  excerpt: HoverCardExcerpt,
): (() => void) => {
  const view = new EditorView({
    state: EditorState.create({
      doc: excerpt.code,
      extensions: [
        EditorState.readOnly.of(true),
        EditorView.editable.of(false),
        resolveLanguage(excerpt.language ?? 'text'),
        resolveEditorTheme(excerpt.theme ?? 'dark'),
        EditorView.lineWrapping,
      ],
    }),
    parent: container,
  });
  return () => view.destroy();
};

const buildCardDom = (card: HoverCard): { dom: HTMLElement; destroy?: () => void } => {
  const root = document.createElement('div');
  root.className = 'cm-napl-card';
  const cleanups: Array<() => void> = [];
  if (card.heading) {
    const heading = document.createElement('div');
    heading.className = 'cm-napl-card__heading';
    heading.textContent = card.heading;
    root.appendChild(heading);
  }
  if (card.quote) {
    const quote = document.createElement('blockquote');
    quote.className = 'cm-napl-card__quote';
    quote.textContent = card.quote;
    root.appendChild(quote);
  }
  if (card.excerpt) {
    const excerptHost = document.createElement('div');
    excerptHost.className = 'cm-napl-card__excerpt';
    root.appendChild(excerptHost);
    cleanups.push(mountExcerpt(excerptHost, card.excerpt));
    if (card.excerpt.caption) {
      const caption = document.createElement('div');
      caption.className = 'cm-napl-card__caption';
      caption.textContent = card.excerpt.caption;
      root.appendChild(caption);
    }
  }
  if (card.meta) {
    const meta = document.createElement('div');
    meta.className = 'cm-napl-card__meta';
    meta.textContent = card.meta;
    root.appendChild(meta);
  }
  if (card.jump) {
    const button = document.createElement('button');
    button.type = 'button';
    button.className = 'cm-napl-card__jump';
    button.textContent = card.jump.label;
    button.tabIndex = 0;
    const run = card.jump.onJump;
    button.addEventListener('click', (event) => {
      event.preventDefault();
      run();
    });
    button.addEventListener('keydown', (event) => {
      if (event.key === 'Enter' || event.key === ' ') {
        event.preventDefault();
        run();
      }
    });
    root.appendChild(button);
  }
  return {
    dom: root,
    destroy: cleanups.length
      ? () => {
          for (const cleanup of cleanups) cleanup();
        }
      : undefined,
  };
};

const paneTooltipSpace = tooltips({
  tooltipSpace: (view) => {
    const rect = view.scrollDOM.getBoundingClientRect();
    return {
      top: rect.top + 4,
      left: 8,
      bottom: rect.bottom - 4,
      right: window.innerWidth - 8,
    };
  },
});

const hoverExtension = (source: HoverSource): Extension =>
  hoverTooltip(
    async (view, pos) => {
      const line = view.state.doc.lineAt(pos);
      const result = await source({
        line: line.number - 1,
        column: pos - line.from,
        content: view.state.doc.toString(),
      });
      if (!result) {
        return null;
      }
      return {
        pos,
        above: true,
        create: () => {
          if (typeof result === 'string') {
            const dom = document.createElement('div');
            dom.className = 'cm-napl-tooltip';
            dom.textContent = result;
            return { dom };
          }
          return buildCardDom(result);
        },
      };
    },
    { hoverTime: 120 },
  );

export const naplHover = (source: HoverSource): Extension => [
  paneTooltipSpace,
  hoverExtension(source),
];
