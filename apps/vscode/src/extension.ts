import { commands, Range, Selection, Uri, window, workspace } from 'vscode';
import type { ExtensionContext } from 'vscode';
import { LanguageClient, TransportKind } from 'vscode-languageclient/node.js';
import type { LanguageClientOptions, ServerOptions } from 'vscode-languageclient/node.js';

let client: LanguageClient | undefined;

interface LspRange {
  start: { line: number; character: number };
  end: { line: number; character: number };
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

function resolveCliPath(): string {
  const configured = workspace.getConfiguration('napl').get<string>('cliPath', 'napl');
  return configured.length > 0 ? configured : 'napl';
}

export function activate(context: ExtensionContext): void {
  context.subscriptions.push(
    commands.registerCommand('napl.revealLocation', (uriString: string, range: LspRange) =>
      revealLocation(uriString, range),
    ),
  );

  const command = resolveCliPath();
  const serverOptions: ServerOptions = {
    run: { command, args: ['lsp'], transport: TransportKind.stdio },
    debug: { command, args: ['lsp'], transport: TransportKind.stdio },
  };
  const clientOptions: LanguageClientOptions = {
    documentSelector: [
      { scheme: 'file', language: 'napl' },
      { scheme: 'file', pattern: '**/*.mapl' },
      { scheme: 'file', pattern: '**/*.\u{1F916}' },
      { scheme: 'file', pattern: '**/.napl/src/**' },
    ],
  };
  client = new LanguageClient('napl', 'NAPL', serverOptions, clientOptions);
  void client.start();
}

export function deactivate(): Thenable<void> | undefined {
  return client?.stop();
}
