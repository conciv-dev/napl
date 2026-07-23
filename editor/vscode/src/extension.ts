import { join } from 'node:path';
import { commands, Range, Selection, StatusBarAlignment, Uri, window, workspace } from 'vscode';
import type { ExtensionContext, StatusBarItem } from 'vscode';
import { LanguageClient, TransportKind } from 'vscode-languageclient/node.js';
import type { LanguageClientOptions, ServerOptions } from 'vscode-languageclient/node.js';

let client: LanguageClient | undefined;
let statusBar: StatusBarItem | undefined;
let errorTimer: ReturnType<typeof setTimeout> | undefined;

interface LspRange {
  start: { line: number; character: number };
  end: { line: number; character: number };
}

interface GenStatus {
  module: string;
  state: 'running' | 'done' | 'error';
  message?: string;
}

async function revealLocation(uriString: string, range: LspRange): Promise<void> {
  const document = await workspace.openTextDocument(Uri.parse(uriString));
  const editor = await window.showTextDocument(document);
  const target = new Range(
    range.start.line,
    range.start.character,
    range.end.line,
    range.end.character,
  );
  editor.selection = new Selection(target.start, target.end);
  editor.revealRange(target);
}

function readConfig(): { genOnSave: boolean; cliPath: string } {
  const config = workspace.getConfiguration('humanLanguage');
  return {
    genOnSave: config.get<boolean>('genOnSave', true),
    cliPath: config.get<string>('cliPath', 'hl'),
  };
}

function showGenStatus(status: GenStatus): void {
  if (statusBar === undefined) return;
  if (errorTimer !== undefined) {
    clearTimeout(errorTimer);
    errorTimer = undefined;
  }
  if (status.state === 'running') {
    statusBar.text = `$(sync~spin) HL: compiling ${status.module}…`;
    statusBar.tooltip = 'Human Language is regenerating code from the saved prompt.';
    statusBar.show();
    return;
  }
  if (status.state === 'error') {
    statusBar.text = `$(error) HL: ${status.module} failed`;
    statusBar.tooltip = status.message ?? 'gen-on-save failed';
    statusBar.show();
    errorTimer = setTimeout(() => statusBar?.hide(), 6000);
    return;
  }
  statusBar.hide();
}

export function activate(context: ExtensionContext): void {
  context.subscriptions.push(
    commands.registerCommand('hl.revealLocation', (uriString: string, range: LspRange) =>
      revealLocation(uriString, range),
    ),
  );

  statusBar = window.createStatusBarItem(StatusBarAlignment.Left, 100);
  context.subscriptions.push(statusBar);

  const serverModule = context.asAbsolutePath(join('dist', 'server.js'));
  const serverOptions: ServerOptions = {
    run: { module: serverModule, transport: TransportKind.ipc },
    debug: { module: serverModule, transport: TransportKind.ipc },
  };
  const clientOptions: LanguageClientOptions = {
    documentSelector: [
      { scheme: 'file', language: 'hl' },
      { scheme: 'file', pattern: '**/.hl/src/**' },
    ],
    initializationOptions: readConfig(),
  };
  client = new LanguageClient('hl', 'Human Language', serverOptions, clientOptions);

  void client.start().then(() => {
    client?.onNotification('hl/genStatus', (status: GenStatus) => showGenStatus(status));
  });

  context.subscriptions.push(
    workspace.onDidChangeConfiguration((event) => {
      if (event.affectsConfiguration('humanLanguage')) {
        void client?.sendNotification('hl/config', readConfig());
      }
    }),
  );
}

export function deactivate(): Thenable<void> | undefined {
  if (errorTimer !== undefined) clearTimeout(errorTimer);
  return client?.stop();
}
