import { spawn } from 'node:child_process';
import { existsSync } from 'node:fs';
import { readdir, readFile } from 'node:fs/promises';
import { basename, dirname, extname, join, relative, sep } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { parseDocument } from 'yaml';
import {
  createConnection,
  DiagnosticSeverity,
  MarkupKind,
  ProposedFeatures,
  TextDocuments,
  TextDocumentSyncKind,
} from 'vscode-languageserver/node.js';
import type {
  CodeLens,
  Definition,
  Diagnostic,
  Hover,
  InitializeResult,
  Location,
  TextDocumentPositionParams,
} from 'vscode-languageserver/node.js';
import { TextDocument } from 'vscode-languageserver-textdocument';
import { validateAttribution } from '@napl/core';
import { entriesAtBodyLine } from '@napl/core';
import type { Attribution, AttributionEntry } from '@napl/core';
import { blameFile, firstPromptDiffLine } from '@napl/core';
import type { BlameLine } from '@napl/core';
import { fileHistory, readJournal } from '@napl/core';
import { mlEntriesAtBodyLine, validateMl } from '@napl/core';
import { machineExtensions } from '@napl/core';
import type { Ml } from '@napl/core';
import { mlDiagnostics, mlHoverMarkdown } from './ml.js';
import { bodyLineForDocLine, promptBodyLines } from '@napl/core';
import { parseFrontmatter } from '@napl/core';
import { contentHash } from '@napl/core';
import { validateIr } from '@napl/core';
import type { Ir } from '@napl/core';
import {
  declaredTargetsForModule,
  filesForModule,
  hasModule,
  promptsForModule,
  readMap,
} from '@napl/core';
import type { NaplMap, ModuleFile } from '@napl/core';
import { classifyPrompt } from '@napl/core';
import {
  buildSpawnCommand,
  decideTriggers,
  initialModuleState,
  reduceSave,
} from './save-trigger.js';
import type { ModuleState, SaveEffects, SaveEvent, TargetTriggerState } from './save-trigger.js';
import {
  codeLensTitle,
  dedupeMatches,
  DRIFT_LENS_PREFIX,
  isFileDrifted,
  parseGeneratedPath,
  promptAbsoluteLines,
  reverseMatches,
} from './reverse.js';
import type { AttributionSource, GeneratedPathInfo, ReverseMatch } from './reverse.js';
import { findTargetAtPosition, scanDocument } from './scanner.js';
import type { Span, Target } from './scanner.js';

const HOVER_CODE_LINES = 40;

export function findWorkspaceRoot(startPath: string): string | null {
  let dir = dirname(startPath);
  for (;;) {
    if (existsSync(join(dir, '.napl'))) return dir;
    const parent = dirname(dir);
    if (parent === dir) return null;
    dir = parent;
  }
}

function isTestFile(filePath: string): boolean {
  return /\.test\.[a-z]+$/i.test(filePath);
}

function orderedFiles(files: ModuleFile[]): ModuleFile[] {
  return [...files].sort((a, b) => Number(isTestFile(a.filePath)) - Number(isTestFile(b.filePath)));
}

function fenceLang(filePath: string): string {
  const ext = extname(filePath).toLowerCase();
  if (ext === '.ts' || ext === '.tsx') return 'ts';
  if (ext === '.js' || ext === '.jsx') return 'js';
  if (ext === '.css') return 'css';
  if (ext === '.html') return 'html';
  return 'text';
}

async function loadIr(root: string, module: string): Promise<Ir | null> {
  const irPath = join(root, '.napl', 'ir', `${module}.yaml`);
  if (!existsSync(irPath)) return null;
  try {
    const raw = await readFile(irPath, 'utf8');
    return validateIr(parseDocument(raw).toJSON());
  } catch {
    return null;
  }
}

async function loadAttribution(root: string, module: string): Promise<Attribution | null> {
  const path = join(root, '.napl', 'attribution', `${module}.yaml`);
  if (!existsSync(path)) return null;
  try {
    const raw = await readFile(path, 'utf8');
    return validateAttribution(parseDocument(raw).toJSON());
  } catch {
    return null;
  }
}

async function loadMl(root: string, module: string): Promise<Ml | null> {
  for (const ext of machineExtensions()) {
    const path = join(root, '.napl', 'mapl', `${module}${ext}`);
    if (!existsSync(path)) continue;
    try {
      const raw = await readFile(path, 'utf8');
      return validateMl(parseDocument(raw).toJSON());
    } catch {
      return null;
    }
  }
  return null;
}

function attributionFileAbs(root: string, attribution: Attribution, entry: AttributionEntry): string {
  return join(root, '.napl', 'src', attribution.target, entry.file);
}

async function buildHoverMarkdown(root: string, module: string): Promise<string> {
  const map = await readMap(join(root, '.napl', 'map.json'));
  if (!hasModule(map, module)) {
    return `**module \`${module}\`** — not generated yet — run \`napl gen\`.`;
  }

  const lines: string[] = [];
  lines.push(`### module \`${module}\``);
  const declared = declaredTargetsForModule(map, module);
  lines.push(`**targets:** ${declared.length > 0 ? declared.join(', ') : '(none)'}`);

  const ir = await loadIr(root, module);
  if (ir !== null && ir.functions.length > 0) {
    lines.push('');
    lines.push('**signatures (IR)**');
    for (const fn of ir.functions) {
      lines.push(`- \`${fn.signature}\``);
    }
  }

  const files = orderedFiles(filesForModule(map, module));
  if (files.length === 0) {
    lines.push('');
    lines.push('_no generated code yet — run `napl gen`._');
    return lines.join('\n');
  }

  const impl = files[0];
  const abs = join(root, impl.filePath);
  if (!existsSync(abs)) {
    lines.push('');
    lines.push(`_generated file missing: \`${impl.filePath}\` — run \`napl gen\`._`);
    return lines.join('\n');
  }

  const code = await readFile(abs, 'utf8');
  const codeLines = code.split(/\r?\n/);
  const snippet = codeLines.slice(0, HOVER_CODE_LINES).join('\n');
  const truncated = codeLines.length > HOVER_CODE_LINES ? '\n…' : '';
  lines.push('');
  lines.push(`**generated (${impl.target}) — \`${impl.filePath}\`**`);
  lines.push('```' + fenceLang(impl.filePath));
  lines.push(snippet + truncated);
  lines.push('```');
  return lines.join('\n');
}

async function buildAttributionHover(
  root: string,
  attribution: Attribution,
  entries: AttributionEntry[],
): Promise<string | null> {
  const lines: string[] = [];
  for (const entry of entries) {
    const abs = attributionFileAbs(root, attribution, entry);
    lines.push(`**${entry.note || 'implemented by'}** — \`${entry.file}\` lines ${entry.lines[0]}–${entry.lines[1]}`);
    if (existsSync(abs)) {
      const code = (await readFile(abs, 'utf8')).split(/\r?\n/);
      const snippet = code.slice(entry.lines[0] - 1, entry.lines[1]).join('\n');
      lines.push('```' + fenceLang(entry.file));
      lines.push(snippet);
      lines.push('```');
    }
  }
  return lines.length > 0 ? lines.join('\n') : null;
}

function spanToRange(span: Span): {
  start: { line: number; character: number };
  end: { line: number; character: number };
} {
  return { start: span.start, end: span.end };
}

interface BodyContext {
  module: string;
  bodyLine: number;
}

function resolveBodyContext(text: string, position: { line: number; character: number }): BodyContext | null {
  let module: string;
  try {
    module = parseFrontmatter(text).frontmatter.module;
  } catch {
    return null;
  }
  const body = promptBodyLines(text);
  const bodyLine = bodyLineForDocLine(body, position.line);
  if (bodyLine === null) return null;
  return { module, bodyLine };
}

async function resolveTarget(
  document: TextDocument,
  params: TextDocumentPositionParams,
): Promise<{ root: string; target: Target } | null> {
  const fsPath = fileURLToPath(document.uri);
  const root = findWorkspaceRoot(fsPath);
  if (root === null) return null;
  const scan = scanDocument(document.getText());
  const target = findTargetAtPosition(scan, params.position);
  if (target === null) return null;
  return { root, target };
}

async function loadAttributionSources(root: string, map: NaplMap): Promise<AttributionSource[]> {
  const dir = join(root, '.napl', 'attribution');
  let names: string[];
  try {
    names = (await readdir(dir)).filter((name) => name.endsWith('.yaml'));
  } catch {
    return [];
  }
  const sources: AttributionSource[] = [];
  for (const name of names) {
    const attribution = await loadAttribution(root, name.slice(0, -'.yaml'.length));
    if (attribution === null) continue;
    sources.push({
      module: attribution.module,
      target: attribution.target,
      entries: attribution.entries,
      promptFiles: promptsForModule(map, attribution.module),
    });
  }
  return sources;
}

interface GeneratedContext {
  root: string;
  relFull: string;
  info: GeneratedPathInfo;
  map: NaplMap;
  sources: AttributionSource[];
}

async function resolveGeneratedContext(document: TextDocument): Promise<GeneratedContext | null> {
  const fsPath = fileURLToPath(document.uri);
  const root = findWorkspaceRoot(fsPath);
  if (root === null) return null;
  const relFull = relative(root, fsPath).split(sep).join('/');
  const info = parseGeneratedPath(relFull);
  if (info === null) return null;
  const map = await readMap(join(root, '.napl', 'map.json'));
  const sources = await loadAttributionSources(root, map);
  return { root, relFull, info, map, sources };
}

async function promptLocation(root: string, match: ReverseMatch): Promise<Location | null> {
  const abs = join(root, match.promptFile);
  if (!existsSync(abs)) return null;
  const text = await readFile(abs, 'utf8');
  const body = promptBodyLines(text);
  const [startLine, endLine] = promptAbsoluteLines(body.bodyStartLine, match.promptLines);
  const docLines = text.split(/\r?\n/);
  const endChar = docLines[endLine]?.length ?? 0;
  return {
    uri: pathToFileURL(abs).href,
    range: { start: { line: startLine, character: 0 }, end: { line: endLine, character: endChar } },
  };
}

function fileLevelPromptFallback(ctx: GeneratedContext): Location[] {
  const record = ctx.map.files[ctx.relFull];
  if (record === undefined) return [];
  const locations: Location[] = [];
  for (const promptFile of record.prompts) {
    const abs = join(ctx.root, promptFile);
    if (!existsSync(abs)) continue;
    locations.push({
      uri: pathToFileURL(abs).href,
      range: { start: { line: 0, character: 0 }, end: { line: 0, character: 0 } },
    });
  }
  return locations;
}

interface MechanicalContext {
  blamed: BlameLine[];
  promptDiffByGen: Map<number, string>;
}

async function loadMechanical(root: string, info: GeneratedPathInfo): Promise<MechanicalContext | null> {
  const entries = await readJournal(join(root, '.napl', 'journal.jsonl'));
  if (entries.length === 0) return null;
  const relPath = `.napl/src/${info.target}/${info.targetRelPath}`;
  const history = fileHistory(entries, relPath);
  if (history.length === 0) return null;
  const abs = join(root, relPath);
  if (!existsSync(abs)) return null;
  const content = await readFile(abs, 'utf8');
  return {
    blamed: blameFile(history, content),
    promptDiffByGen: new Map(history.map((entry) => [entry.gen, entry.promptDiff])),
  };
}

function mechanicalLabel(mechanical: MechanicalContext, line1: number): string | null {
  const blamed = mechanical.blamed.find((entry) => entry.line === line1);
  if (blamed === undefined) return null;
  const first = firstPromptDiffLine(mechanical.promptDiffByGen.get(blamed.gen) ?? '');
  const date = blamed.timestamp.slice(0, 10);
  const edit = first === '' ? 'initial generation' : `prompt edit: ${first}`;
  return `caused by gen #${blamed.gen} · ${date} · ${edit}`;
}

async function reverseHover(ctx: GeneratedContext, line: number): Promise<Hover | null> {
  const matches = reverseMatches(ctx.sources, ctx.info.target, ctx.info.targetRelPath, line + 1);
  const mechanical = await loadMechanical(ctx.root, ctx.info);
  const mechLabel = mechanical !== null ? mechanicalLabel(mechanical, line + 1) : null;
  if (matches.length === 0 && mechLabel === null) return null;
  const blocks: string[] = [];
  if (mechLabel !== null) {
    blocks.push(`**${mechLabel}**`);
    blocks.push('');
  }
  for (const match of dedupeMatches(matches)) {
    const abs = join(ctx.root, match.promptFile);
    if (!existsSync(abs)) continue;
    const text = await readFile(abs, 'utf8');
    const body = promptBodyLines(text);
    const [startLine, endLine] = promptAbsoluteLines(body.bodyStartLine, match.promptLines);
    const docLines = text.split(/\r?\n/);
    const sentence = docLines.slice(startLine, endLine + 1).join('\n');
    blocks.push(`**⇠ ${basename(match.promptFile)}:${startLine + 1}** — ${match.note || 'prompt'}`);
    blocks.push(sentence.split('\n').map((s) => `> ${s}`).join('\n'));
    blocks.push('');
  }
  if (blocks.length === 0) return null;
  return { contents: { kind: MarkupKind.Markdown, value: blocks.join('\n') } };
}

async function reverseDefinition(ctx: GeneratedContext, line: number): Promise<Definition | null> {
  const matches = dedupeMatches(
    reverseMatches(ctx.sources, ctx.info.target, ctx.info.targetRelPath, line + 1),
  );
  const locations: Location[] = [];
  for (const match of matches) {
    const location = await promptLocation(ctx.root, match);
    if (location !== null) locations.push(location);
  }
  if (locations.length === 0) {
    const fallback = fileLevelPromptFallback(ctx);
    return fallback.length > 0 ? fallback : null;
  }
  return locations;
}

async function reverseReferences(ctx: GeneratedContext, line: number): Promise<Location[] | null> {
  let matches = reverseMatches(ctx.sources, ctx.info.target, ctx.info.targetRelPath, line + 1);
  if (matches.length === 0) {
    matches = reverseMatches(ctx.sources, ctx.info.target, ctx.info.targetRelPath, null);
  }
  const locations: Location[] = [];
  for (const match of dedupeMatches(matches)) {
    const location = await promptLocation(ctx.root, match);
    if (location !== null) locations.push(location);
  }
  return locations.length > 0 ? locations : null;
}

async function reverseCodeLenses(ctx: GeneratedContext, currentText: string): Promise<CodeLens[]> {
  const matches = reverseMatches(ctx.sources, ctx.info.target, ctx.info.targetRelPath, null);
  if (matches.length === 0) return [];
  const record = ctx.map.files[ctx.relFull];
  const drifted = isFileDrifted(record?.hash, contentHash(currentText));
  const mechanical = await loadMechanical(ctx.root, ctx.info);
  const sorted = [...matches].sort((a, b) => a.codeLines[0] - b.codeLines[0]);
  const lenses: CodeLens[] = [];
  let driftApplied = false;
  for (const match of sorted) {
    const location = await promptLocation(ctx.root, match);
    if (location === null) continue;
    const semantic = codeLensTitle(basename(match.promptFile), location.range.start.line + 1, match.note);
    const mechLabel = mechanical !== null ? mechanicalLabel(mechanical, match.codeLines[0]) : null;
    const base = mechLabel !== null ? `${mechLabel}   ${semantic}` : semantic;
    const title = drifted && !driftApplied ? `${DRIFT_LENS_PREFIX}   ${base}` : base;
    if (drifted) driftApplied = true;
    const anchor = match.codeLines[0] - 1;
    lenses.push({
      range: { start: { line: anchor, character: 0 }, end: { line: anchor, character: 0 } },
      command: { title, command: 'napl.revealLocation', arguments: [location.uri, location.range] },
    });
  }
  return lenses;
}

async function computeDiagnostics(root: string, relPath: string, text: string): Promise<Diagnostic[]> {
  const diagnostics: Diagnostic[] = [];
  const scan = scanDocument(text);

  if (scan.frontmatter.present && scan.frontmatter.span !== null) {
    const innerStartLine = scan.frontmatter.span.start.line;
    const innerLines = text.split(/\r?\n/).slice(innerStartLine, scan.frontmatter.span.end.line + 1);
    const doc = parseDocument(innerLines.join('\n'));
    for (const error of doc.errors) {
      const line = error.linePos ? innerStartLine + (error.linePos[0].line - 1) : innerStartLine;
      const character = error.linePos ? Math.max(0, error.linePos[0].col - 1) : 0;
      diagnostics.push({
        severity: DiagnosticSeverity.Error,
        range: { start: { line, character }, end: { line, character: character + 1 } },
        message: `YAML frontmatter error: ${error.message}`,
        source: 'napl',
      });
    }
    if (doc.errors.length > 0) return diagnostics;
  } else {
    diagnostics.push({
      severity: DiagnosticSeverity.Error,
      range: { start: { line: 0, character: 0 }, end: { line: 0, character: 1 } },
      message: 'missing YAML frontmatter: a prompt file must start with a --- delimited block',
      source: 'napl',
    });
    return diagnostics;
  }

  const map = await readMap(join(root, '.napl', 'map.json'));
  let entry;
  try {
    entry = await classifyPrompt({ root, relPath, raw: text, map });
  } catch (cause) {
    diagnostics.push({
      severity: DiagnosticSeverity.Error,
      range: { start: { line: 0, character: 0 }, end: { line: 0, character: 3 } },
      message: cause instanceof Error ? cause.message : String(cause),
      source: 'napl',
    });
    return diagnostics;
  }

  const firstLine = { start: { line: 0, character: 0 }, end: { line: 0, character: 3 } };
  if (entry.status === 'DRIFT') {
    const driftTarget = entry.detail.includes(':') ? entry.detail.slice(0, entry.detail.indexOf(':')) : entry.detail;
    diagnostics.push({
      severity: DiagnosticSeverity.Error,
      range: firstLine,
      message:
        `DRIFT: generated file ${entry.driftFile ?? entry.detail} was edited — it no longer matches the prompt. ` +
        `Resolve: (1) napl reconcile ${entry.module} to fold the edit into your prompt (coming soon); ` +
        `(2) napl gen ${driftTarget} --module ${entry.module} --force to discard the edit; ` +
        `(3) edit the prompt to describe the change, then napl gen ${driftTarget}.`,
      source: 'napl',
    });
  } else if (entry.status === 'prompt-stale') {
    diagnostics.push({
      severity: DiagnosticSeverity.Information,
      range: firstLine,
      message: `prompt changed since last gen (${entry.detail}) — run napl gen`,
      source: 'napl',
    });
  }

  await appendMlDiagnostics(root, text, diagnostics);

  return diagnostics;
}

async function appendMlDiagnostics(root: string, text: string, diagnostics: Diagnostic[]): Promise<void> {
  let module: string;
  try {
    module = parseFrontmatter(text).frontmatter.module;
  } catch {
    return;
  }
  const ml = await loadMl(root, module);
  if (ml === null) return;
  const body = promptBodyLines(text);
  const docLines = text.split(/\r?\n/);
  for (const diagnostic of mlDiagnostics(ml, body.bodyStartLine, docLines)) {
    diagnostics.push(diagnostic);
  }
}

const connection = createConnection(ProposedFeatures.all);
const documents = new TextDocuments(TextDocument);

const DEBOUNCE_MS = 1500;

interface SaveConfig {
  genOnSave: boolean;
  cliPath: string;
}

const saveConfig: SaveConfig = { genOnSave: true, cliPath: 'napl' };

interface ModuleRun {
  state: ModuleState;
  timer: ReturnType<typeof setTimeout> | null;
  root: string;
  docUri: string;
  targets: string[];
}

const moduleRuns = new Map<string, ModuleRun>();

connection.onInitialize((params): InitializeResult => {
  const opts = params.initializationOptions as { genOnSave?: boolean; cliPath?: string } | undefined;
  if (opts !== undefined) {
    if (typeof opts.genOnSave === 'boolean') saveConfig.genOnSave = opts.genOnSave;
    if (typeof opts.cliPath === 'string' && opts.cliPath.length > 0) saveConfig.cliPath = opts.cliPath;
  }
  return {
    capabilities: {
      textDocumentSync: TextDocumentSyncKind.Incremental,
      hoverProvider: true,
      definitionProvider: true,
      referencesProvider: true,
      codeLensProvider: { resolveProvider: false },
    },
  };
});

connection.onNotification('napl/config', (cfg: { genOnSave?: boolean; cliPath?: string }) => {
  if (typeof cfg.genOnSave === 'boolean') saveConfig.genOnSave = cfg.genOnSave;
  if (typeof cfg.cliPath === 'string' && cfg.cliPath.length > 0) saveConfig.cliPath = cfg.cliPath;
});

function sendGenStatus(module: string, state: 'running' | 'done' | 'error', message?: string): void {
  connection.sendNotification('napl/genStatus', { module, state, message });
}

function refreshDiagnostics(): void {
  for (const doc of documents.all()) void validate(doc);
}

function runGenChild(root: string, cliPath: string, target: string, module: string): Promise<number> {
  const { command, args } = buildSpawnCommand(cliPath, process.execPath, target, module);
  return new Promise((resolve) => {
    const child = spawn(command, args, { cwd: root, stdio: ['ignore', 'pipe', 'pipe'] });
    let tail = '';
    const capture = (chunk: Buffer): void => {
      tail = (tail + chunk.toString()).slice(-4000);
    };
    child.stdout.on('data', capture);
    child.stderr.on('data', capture);
    child.on('error', () => {
      connection.console.error(`napl gen ${target} --module ${module} failed to spawn`);
      resolve(1);
    });
    child.on('close', (code) => {
      if (code !== 0) connection.console.error(`napl gen ${target} --module ${module} exited ${code ?? 1}:\n${tail}`);
      resolve(code ?? 1);
    });
  });
}

async function performRun(module: string): Promise<void> {
  const run = moduleRuns.get(module);
  if (run === undefined) return;
  const targets = [...run.targets];
  sendGenStatus(module, 'running', `compiling ${module}…`);
  connection.console.log(`napl: gen-on-save running for ${module} [${targets.join(', ')}]`);
  let failure: string | null = null;
  for (const target of targets) {
    const code = await runGenChild(run.root, saveConfig.cliPath, target, module);
    if (code !== 0) failure = `${target} exited ${code}`;
  }
  refreshDiagnostics();
  if (failure === null) sendGenStatus(module, 'done');
  else sendGenStatus(module, 'error', failure);
  dispatch(module, { type: 'runFinished' });
}

function applyEffects(module: string, effects: SaveEffects): void {
  const run = moduleRuns.get(module);
  if (run === undefined) return;
  if (effects.startDebounce === true) {
    if (run.timer !== null) clearTimeout(run.timer);
    run.timer = setTimeout(() => {
      run.timer = null;
      dispatch(module, { type: 'debounceElapsed' });
    }, DEBOUNCE_MS);
  }
  if (effects.startRun === true) {
    void performRun(module);
  }
}

function dispatch(module: string, event: SaveEvent): void {
  const run = moduleRuns.get(module);
  if (run === undefined) return;
  const { state, effects } = reduceSave(run.state, event);
  run.state = state;
  applyEffects(module, effects);
}

async function triggerGenOnSave(document: TextDocument): Promise<void> {
  if (!saveConfig.genOnSave) return;
  const fsPath = fileURLToPath(document.uri);
  const root = findWorkspaceRoot(fsPath);
  if (root === null) return;
  const text = document.getText();
  let frontmatter;
  try {
    frontmatter = parseFrontmatter(text).frontmatter;
  } catch {
    return;
  }
  if (frontmatter.targets.length === 0) return;

  const relPath = relative(root, fsPath).split(sep).join('/');
  const map = await readMap(join(root, '.napl', 'map.json'));
  const record = map.prompts[relPath];
  const currentPromptHash = contentHash(text);
  const targetStates: TargetTriggerState[] = frontmatter.targets.map((target) => ({
    target,
    currentPromptHash,
    promptHashAtGen: record?.targets[target]?.promptHashAtGen,
  }));

  const actions = decideTriggers({ enabled: saveConfig.genOnSave, module: frontmatter.module, targets: targetStates });
  if (actions.length === 0) return;

  const module = frontmatter.module;
  const existing = moduleRuns.get(module);
  const runState = existing ?? { state: initialModuleState(), timer: null, root, docUri: document.uri, targets: [] };
  runState.root = root;
  runState.docUri = document.uri;
  runState.targets = actions.map((action) => action.target);
  moduleRuns.set(module, runState);

  dispatch(module, { type: 'save' });
}

connection.onHover(async (params): Promise<Hover | null> => {
  const document = documents.get(params.textDocument.uri);
  if (document === undefined) return null;

  const generated = await resolveGeneratedContext(document);
  if (generated !== null) return reverseHover(generated, params.position.line);

  const resolved = await resolveTarget(document, params);
  if (resolved !== null) {
    const markdown = await buildHoverMarkdown(resolved.root, resolved.target.module);
    return {
      contents: { kind: MarkupKind.Markdown, value: markdown },
      range: spanToRange(resolved.target.span),
    };
  }

  const fsPath = fileURLToPath(document.uri);
  const root = findWorkspaceRoot(fsPath);
  if (root === null) return null;
  const context = resolveBodyContext(document.getText(), params.position);
  if (context === null) return null;
  const attribution = await loadAttribution(root, context.module);
  const ml = await loadMl(root, context.module);
  const attrEntries = attribution !== null ? entriesAtBodyLine(attribution, context.bodyLine) : [];
  const mlEntries = ml !== null ? mlEntriesAtBodyLine(ml, context.bodyLine) : [];
  if (attrEntries.length === 0 && mlEntries.length === 0) return null;

  const sections: string[] = [];
  if (attribution !== null && attrEntries.length > 0) {
    const attrMarkdown = await buildAttributionHover(root, attribution, attrEntries);
    if (attrMarkdown !== null) sections.push(attrMarkdown);
  }
  if (mlEntries.length > 0) sections.push(mlHoverMarkdown(mlEntries));
  if (sections.length === 0) return null;

  const rangeEntries = [...attrEntries, ...mlEntries];
  const bodyStart = promptBodyLines(document.getText()).bodyStartLine;
  const startLine = bodyStart + Math.min(...rangeEntries.map((e) => e.promptLines[0])) - 1;
  const endLine = bodyStart + Math.max(...rangeEntries.map((e) => e.promptLines[1])) - 1;
  return {
    contents: { kind: MarkupKind.Markdown, value: sections.join('\n\n---\n\n') },
    range: { start: { line: startLine, character: 0 }, end: { line: endLine, character: 200 } },
  };
});

connection.onDefinition(async (params): Promise<Definition | null> => {
  const document = documents.get(params.textDocument.uri);
  if (document === undefined) return null;

  const generated = await resolveGeneratedContext(document);
  if (generated !== null) return reverseDefinition(generated, params.position.line);

  const resolved = await resolveTarget(document, params);
  if (resolved !== null) {
    const { root, target } = resolved;
    const map = await readMap(join(root, '.napl', 'map.json'));
    if (!hasModule(map, target.module)) return null;
    const locations: Location[] = [];
    for (const { filePath } of orderedFiles(filesForModule(map, target.module))) {
      const abs = join(root, filePath);
      if (!existsSync(abs)) continue;
      locations.push({
        uri: pathToFileURL(abs).href,
        range: { start: { line: 0, character: 0 }, end: { line: 0, character: 0 } },
      });
    }
    return locations;
  }

  const fsPath = fileURLToPath(document.uri);
  const root = findWorkspaceRoot(fsPath);
  if (root === null) return null;
  const context = resolveBodyContext(document.getText(), params.position);
  if (context === null) return null;
  const attribution = await loadAttribution(root, context.module);
  if (attribution === null) return null;
  const entries = entriesAtBodyLine(attribution, context.bodyLine);
  if (entries.length === 0) return null;

  const locations: Location[] = [];
  for (const entry of entries) {
    const abs = attributionFileAbs(root, attribution, entry);
    if (!existsSync(abs)) continue;
    const code = (await readFile(abs, 'utf8')).split(/\r?\n/);
    const endLineIndex = Math.min(entry.lines[1], code.length) - 1;
    const endChar = code[endLineIndex]?.length ?? 0;
    locations.push({
      uri: pathToFileURL(abs).href,
      range: {
        start: { line: entry.lines[0] - 1, character: 0 },
        end: { line: endLineIndex, character: endChar },
      },
    });
  }
  return locations;
});

connection.onReferences(async (params): Promise<Location[] | null> => {
  const document = documents.get(params.textDocument.uri);
  if (document === undefined) return null;
  const generated = await resolveGeneratedContext(document);
  if (generated === null) return null;
  return reverseReferences(generated, params.position.line);
});

connection.onCodeLens(async (params): Promise<CodeLens[] | null> => {
  const document = documents.get(params.textDocument.uri);
  if (document === undefined) return null;
  const generated = await resolveGeneratedContext(document);
  if (generated === null) return null;
  return reverseCodeLenses(generated, document.getText());
});

async function validate(document: TextDocument): Promise<void> {
  const fsPath = fileURLToPath(document.uri);
  const root = findWorkspaceRoot(fsPath);
  if (root === null) {
    connection.sendDiagnostics({ uri: document.uri, diagnostics: [] });
    return;
  }
  const relPath = relative(root, fsPath);
  if (parseGeneratedPath(relPath.split(sep).join('/')) !== null) {
    connection.sendDiagnostics({ uri: document.uri, diagnostics: [] });
    return;
  }
  const diagnostics = await computeDiagnostics(root, relPath, document.getText());
  connection.sendDiagnostics({ uri: document.uri, diagnostics });
}

documents.onDidOpen((event) => {
  void validate(event.document);
});

documents.onDidSave((event) => {
  void validate(event.document);
  void triggerGenOnSave(event.document);
});

documents.listen(connection);
connection.listen();
