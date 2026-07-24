import type { Extension } from '@codemirror/state';
import { linter, type Diagnostic } from '@codemirror/lint';
import { EditorView, hoverTooltip } from '@codemirror/view';

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

const buildCardDom = (card: HoverCard): HTMLElement => {
  const root = document.createElement('div');
  root.className = 'cm-napl-card';
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
    const pre = document.createElement('pre');
    pre.className = 'cm-napl-card__excerpt';
    const code = document.createElement('code');
    code.textContent = card.excerpt.code;
    pre.appendChild(code);
    root.appendChild(pre);
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
  return root;
};

export const naplHover = (source: HoverSource): Extension =>
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
          return { dom: buildCardDom(result) };
        },
      };
    },
    { hoverTime: 120 },
  );
