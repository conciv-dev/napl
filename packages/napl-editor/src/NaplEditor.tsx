import {
  Compartment,
  EditorState,
  RangeSetBuilder,
  StateEffect,
  StateField,
  type Extension,
} from '@codemirror/state';
import { Decoration, EditorView } from '@codemirror/view';
import { basicSetup } from 'codemirror';
import { useEffect, useRef, type ReactElement } from 'react';
import {
  naplHover,
  naplLinter,
  type DiagnosticsSource,
  type HoverSource,
} from './editor-extensions.ts';
import { naplLanguage } from './napl-language.ts';

export type HighlightRange = [number, number];

export interface NaplEditorProps {
  value: string;
  onChange?: (value: string) => void;
  readOnly?: boolean;
  diagnostics?: DiagnosticsSource;
  hover?: HoverSource;
  highlightRanges?: HighlightRange[];
  className?: string;
}

const readOnlyExtension = (readOnly: boolean): Extension => [
  EditorState.readOnly.of(readOnly),
  EditorView.editable.of(!readOnly),
];

const setHighlightRanges = StateEffect.define<HighlightRange[]>();

const highlightLineDecoration = Decoration.line({ class: 'cm-napl-linked' });

const highlightField = StateField.define<HighlightRange[]>({
  create: () => [],
  update: (value, transaction) => {
    let next = value;
    for (const effect of transaction.effects) {
      if (effect.is(setHighlightRanges)) {
        next = effect.value;
      }
    }
    return next;
  },
});

const highlightPlugin = EditorView.decorations.compute([highlightField], (state) => {
  const ranges = state.field(highlightField);
  if (ranges.length === 0) {
    return Decoration.none;
  }
  const builder = new RangeSetBuilder<Decoration>();
  const lines = new Set<number>();
  for (const [start, end] of ranges) {
    for (let line = start; line <= end; line += 1) {
      if (line >= 1 && line <= state.doc.lines) {
        lines.add(line);
      }
    }
  }
  for (const line of [...lines].sort((a, b) => a - b)) {
    builder.add(state.doc.line(line).from, state.doc.line(line).from, highlightLineDecoration);
  }
  return builder.finish();
});

export const NaplEditor = ({
  value,
  onChange,
  readOnly = false,
  diagnostics,
  hover,
  highlightRanges,
  className,
}: NaplEditorProps): ReactElement => {
  const hostRef = useRef<HTMLDivElement | null>(null);
  const viewRef = useRef<EditorView | null>(null);
  const onChangeRef = useRef(onChange);
  const diagnosticsRef = useRef(diagnostics);
  const hoverRef = useRef(hover);
  const readOnlyCompartment = useRef(new Compartment());

  onChangeRef.current = onChange;
  diagnosticsRef.current = diagnostics;
  hoverRef.current = hover;

  useEffect(() => {
    const host = hostRef.current;
    if (!host) {
      return undefined;
    }
    const view = new EditorView({
      state: EditorState.create({
        doc: value,
        extensions: [
          basicSetup,
          naplLanguage(),
          highlightField,
          highlightPlugin,
          readOnlyCompartment.current.of(readOnlyExtension(readOnly)),
          EditorView.updateListener.of((update) => {
            if (update.docChanged) {
              onChangeRef.current?.(update.state.doc.toString());
            }
          }),
          naplLinter((content) =>
            diagnosticsRef.current ? diagnosticsRef.current(content) : [],
          ),
          naplHover((context) =>
            hoverRef.current ? hoverRef.current(context) : null,
          ),
        ],
      }),
      parent: host,
    });
    viewRef.current = view;
    return () => {
      view.destroy();
      viewRef.current = null;
    };
  }, []);

  useEffect(() => {
    const view = viewRef.current;
    if (!view) {
      return;
    }
    const current = view.state.doc.toString();
    if (current !== value) {
      view.dispatch({ changes: { from: 0, to: current.length, insert: value } });
    }
  }, [value]);

  useEffect(() => {
    const view = viewRef.current;
    if (!view) {
      return;
    }
    view.dispatch({
      effects: readOnlyCompartment.current.reconfigure(readOnlyExtension(readOnly)),
    });
  }, [readOnly]);

  useEffect(() => {
    const view = viewRef.current;
    if (!view) {
      return;
    }
    view.dispatch({ effects: setHighlightRanges.of(highlightRanges ?? []) });
  }, [highlightRanges]);

  return (
    <div
      ref={hostRef}
      data-testid="napl-editor"
      className={className ? `napl-editor ${className}` : 'napl-editor'}
    />
  );
};
