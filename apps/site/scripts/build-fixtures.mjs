import { existsSync } from 'node:fs';
import { mkdir, readdir, readFile, rm, writeFile } from 'node:fs/promises';
import { basename, dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { initNaplWasm, maplParse } from '@napl/wasm';

const here = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(here, '../../..');
const siteRoot = resolve(here, '..');
const outDir = join(siteRoot, 'src/fixtures');
const modulesDir = join(outDir, 'modules');

const PROMPT_EXTENSIONS = ['.napl', '.🧑'];
const MAPL_EXTENSIONS = ['.mapl', '.🤖'];
const WALK_SKIP = new Set(['node_modules', 'target', '.napl', '.git', 'dist']);

const sources = [
  {
    collection: 'selfhost',
    promptRoot: join(repoRoot, 'selfhost'),
    projectRoot: join(repoRoot, 'selfhost'),
    stateDir: join(repoRoot, 'selfhost/.napl'),
  },
  {
    collection: 'example',
    promptRoot: join(repoRoot, 'examples'),
    projectRoot: repoRoot,
    stateDir: join(repoRoot, '.napl'),
  },
];

const languageForPath = (path) => {
  if (path.endsWith('.rs')) return 'rust';
  if (path.endsWith('.toml')) return 'toml';
  if (path.endsWith('.ts') || path.endsWith('.tsx')) return 'typescript';
  if (path.endsWith('.js') || path.endsWith('.jsx')) return 'javascript';
  if (path.endsWith('.json')) return 'json';
  if (path.endsWith('.md')) return 'markdown';
  if (path.endsWith('.css')) return 'css';
  if (path.endsWith('.html')) return 'html';
  return 'text';
};

const displayGeneratedPath = (filePath) =>
  filePath.replace(/^\.napl\/src\/[^/]+\//, '');

const walkPrompts = async (dir, out) => {
  let entries;
  try {
    entries = await readdir(dir, { withFileTypes: true });
  } catch {
    return out;
  }
  for (const entry of entries) {
    if (WALK_SKIP.has(entry.name)) continue;
    const full = join(dir, entry.name);
    if (entry.isDirectory()) {
      await walkPrompts(full, out);
    } else if (PROMPT_EXTENSIONS.some((ext) => entry.name.endsWith(ext))) {
      out.push(full);
    }
  }
  return out;
};

const readFrontmatter = (content) => {
  const match = /^---\r?\n([\s\S]*?)\r?\n---/.exec(content);
  if (!match) return null;
  const block = match[1];
  const moduleMatch = /^module:\s*(.+?)\s*$/m.exec(block);
  if (!moduleMatch) return null;
  const targetsMatch = /^targets:\s*\[(.*?)\]\s*$/m.exec(block);
  const targets = targetsMatch
    ? targetsMatch[1]
        .split(',')
        .map((value) => value.trim())
        .filter(Boolean)
    : [];
  return { module: moduleMatch[1].trim(), targets };
};

const parseAttribution = (text) => {
  const lines = text.split('\n');
  const entries = [];
  let current = null;
  let context = null;
  const flush = () => {
    if (current) entries.push(current);
    current = null;
  };
  for (const raw of lines) {
    if (/^\s*- promptLines:/.test(raw)) {
      flush();
      current = { promptLines: [], file: '', lines: [], note: '' };
      context = 'promptLines';
      continue;
    }
    if (!current) continue;
    const numberMatch = /^\s*-\s*(-?\d+)\s*$/.exec(raw);
    if (numberMatch && context) {
      current[context].push(Number(numberMatch[1]));
      continue;
    }
    const fileMatch = /^\s*file:\s*(.+?)\s*$/.exec(raw);
    if (fileMatch) {
      current.file = fileMatch[1];
      context = null;
      continue;
    }
    if (/^\s*lines:\s*$/.test(raw)) {
      context = 'lines';
      continue;
    }
    if (/^\s*promptLines:\s*$/.test(raw)) {
      context = 'promptLines';
      continue;
    }
    const noteMatch = /^\s*note:\s*(.*)$/.exec(raw);
    if (noteMatch) {
      current.note = noteMatch[1].replace(/^['"]|['"]$/g, '');
      context = null;
    }
  }
  flush();
  return entries
    .filter((entry) => entry.promptLines.length >= 2 && entry.lines.length >= 2)
    .map((entry) => ({
      promptLines: [entry.promptLines[0], entry.promptLines[1]],
      file: entry.file,
      lines: [entry.lines[0], entry.lines[1]],
      note: entry.note,
    }));
};

const applyUnifiedPatch = (base, patch) => {
  const baseLines = base.length ? base.split('\n') : [];
  const patchLines = patch.split('\n');
  const result = [];
  let cursor = 0;
  let index = 0;
  while (index < patchLines.length) {
    const header = /^@@ -(\d+)(?:,\d+)? \+\d+(?:,\d+)? @@/.exec(patchLines[index]);
    if (!header) {
      index += 1;
      continue;
    }
    const oldStart = Number(header[1]);
    const target = oldStart === 0 ? 0 : oldStart - 1;
    while (cursor < target && cursor < baseLines.length) {
      result.push(baseLines[cursor]);
      cursor += 1;
    }
    index += 1;
    while (index < patchLines.length && !patchLines[index].startsWith('@@')) {
      const line = patchLines[index];
      if (line.startsWith('+')) {
        result.push(line.slice(1));
      } else if (line.startsWith('-')) {
        cursor += 1;
      } else if (line.startsWith(' ')) {
        if (cursor < baseLines.length) result.push(baseLines[cursor]);
        cursor += 1;
      }
      index += 1;
    }
  }
  while (cursor < baseLines.length) {
    result.push(baseLines[cursor]);
    cursor += 1;
  }
  return result.join('\n');
};

const firstProse = (content) => {
  const body = content.replace(/^---\r?\n[\s\S]*?\r?\n---\r?\n?/, '');
  for (const line of body.split('\n')) {
    const trimmed = line.replace(/^#+\s*/, '').trim();
    if (trimmed) return trimmed;
  }
  return '';
};

const buildModule = async (source, promptFile, moduleName, targets, promptContent, map, journalRecords) => {
  const promptEntry = Object.values(map.prompts ?? {}).find(
    (entry) => entry.module === moduleName,
  );
  if (!promptEntry) return null;
  const target = targets[0] ?? Object.keys(promptEntry.targets ?? {})[0];
  if (!target) return null;
  const targetInfo = promptEntry.targets?.[target];
  if (!targetInfo) return null;

  const fileRecords = [];
  for (const relPath of targetInfo.files) {
    const absolute = join(source.projectRoot, relPath);
    let content = '';
    try {
      content = await readFile(absolute, 'utf8');
    } catch {
      content = '';
    }
    fileRecords.push({
      journalPath: relPath,
      path: displayGeneratedPath(relPath),
      content,
      language: languageForPath(relPath),
      hash: map.files?.[relPath]?.hash ?? null,
    });
  }

  const attributionText = await readState(source.stateDir, 'attribution', moduleName, ['.yaml']);
  const attribution = attributionText ? parseAttribution(attributionText) : [];

  const maplText = await readState(source.stateDir, 'mapl', moduleName, MAPL_EXTENSIONS);
  const maplEntries = maplText
    ? maplParse(maplText).map((entry) => ({
        promptLines: [entry.prompt_lines.start, entry.prompt_lines.end],
        kind: entry.kind,
        severity: entry.severity,
        message: entry.message,
        reasoning: entry.reasoning || undefined,
        suggestion: entry.suggestion ?? undefined,
      }))
    : [];

  const promptAtGen = await readState(source.stateDir, 'prompts-at-gen', moduleName, ['.md']);

  const journal = journalRecords
    .filter((record) => record.module === moduleName)
    .sort((a, b) => a.gen - b.gen)
    .map((record) => ({
      gen: record.gen,
      timestamp: record.timestamp,
      mode: record.mode,
      promptHash: record.promptHash,
      files: record.files.map((file) => ({
        path: displayGeneratedPath(file.path),
        journalPath: file.path,
        patch: file.patch,
      })),
    }));

  const diskByJournalPath = new Map(
    fileRecords.map((file) => [file.journalPath, file.content]),
  );
  const lastGenForFile = new Map();
  const orderedRecords = journalRecords
    .filter((record) => record.module === moduleName)
    .sort((a, b) => a.gen - b.gen);
  orderedRecords.forEach((record, recordIndex) => {
    for (const file of record.files) {
      lastGenForFile.set(file.path, recordIndex);
    }
  });

  const events = [];
  events.push({
    type: 'task',
    task: `napl gen ${target} → ${moduleName}`,
  });
  const running = new Map();
  orderedRecords.forEach((record, recordIndex) => {
    for (const file of record.files) {
      const display = displayGeneratedPath(file.path);
      events.push({ type: 'diff', path: display, patch: file.patch });
      const reconstructed = applyUnifiedPatch(running.get(file.path) ?? '', file.patch);
      running.set(file.path, reconstructed);
      const isLast = lastGenForFile.get(file.path) === recordIndex;
      const content = isLast ? diskByJournalPath.get(file.path) ?? reconstructed : reconstructed;
      events.push({ type: 'file-edit', path: display, content });
    }
  });
  if (attribution.length > 0) {
    events.push({ type: 'attribution', module: moduleName, target, entries: attribution });
  }
  for (const entry of maplEntries) {
    events.push({ type: 'mapl-entry', path: `${moduleName}.mapl`, entry });
  }
  events.push({
    type: 'lock',
    module: moduleName,
    target,
    files: fileRecords
      .filter((file) => file.hash)
      .map((file) => ({ path: file.path, hash: file.hash })),
  });

  const promptDisplayName = basename(promptFile);
  const session = {
    task: `napl gen ${target} → ${moduleName}`,
    files: { [promptDisplayName]: promptContent },
    events,
  };

  return {
    module: moduleName,
    collection: source.collection,
    target,
    targets: promptEntry.declaredTargets ?? targets,
    prompt: {
      file: promptDisplayName,
      path: relPathFrom(source.projectRoot, promptFile),
      content: promptContent,
    },
    promptAtGen: promptAtGen ?? null,
    summary: firstProse(promptContent),
    files: fileRecords.map((file) => ({
      path: file.path,
      journalPath: file.journalPath,
      content: file.content,
      language: file.language,
    })),
    attribution,
    attributionYaml: attributionText ?? '',
    mapl: maplEntries,
    journal,
    lock: fileRecords
      .filter((file) => file.hash)
      .map((file) => ({ path: file.path, hash: file.hash })),
    session,
  };
};

const relPathFrom = (root, file) => file.slice(root.length + 1);

const readState = async (stateDir, folder, moduleName, extensions) => {
  for (const ext of extensions) {
    const candidate = join(stateDir, folder, `${moduleName}${ext}`);
    if (existsSync(candidate)) {
      return readFile(candidate, 'utf8');
    }
  }
  return null;
};

const readJournal = async (stateDir) => {
  const journalPath = join(stateDir, 'journal.jsonl');
  if (!existsSync(journalPath)) return [];
  const text = await readFile(journalPath, 'utf8');
  return text
    .split('\n')
    .filter((line) => line.trim().length > 0)
    .map((line) => JSON.parse(line));
};

const run = async () => {
  await initNaplWasm();
  await rm(outDir, { recursive: true, force: true });
  await mkdir(modulesDir, { recursive: true });

  const built = [];
  for (const source of sources) {
    const map = existsSync(join(source.stateDir, 'map.json'))
      ? JSON.parse(await readFile(join(source.stateDir, 'map.json'), 'utf8'))
      : { prompts: {}, files: {} };
    const journalRecords = await readJournal(source.stateDir);
    const promptFiles = await walkPrompts(source.promptRoot, []);
    for (const promptFile of promptFiles) {
      const content = await readFile(promptFile, 'utf8');
      const frontmatter = readFrontmatter(content);
      if (!frontmatter) continue;
      const module = await buildModule(
        source,
        promptFile,
        frontmatter.module,
        frontmatter.targets,
        content,
        map,
        journalRecords,
      );
      if (module) built.push(module);
    }
  }

  built.sort((a, b) => {
    const genA = a.journal[0]?.gen ?? Number.MAX_SAFE_INTEGER;
    const genB = b.journal[0]?.gen ?? Number.MAX_SAFE_INTEGER;
    if (genA !== genB) return genA - genB;
    return a.module.localeCompare(b.module);
  });

  for (const module of built) {
    await writeFile(
      join(modulesDir, `${module.module}.json`),
      `${JSON.stringify(module, null, 2)}\n`,
    );
  }

  const index = built
    .filter((module) => module.collection === 'selfhost')
    .map((module) => ({
      module: module.module,
      target: module.target,
      dir: module.prompt.path.includes('/')
        ? module.prompt.path.slice(0, module.prompt.path.indexOf('/'))
        : 'root',
      promptFile: module.prompt.file,
      fileCount: module.files.length,
      attributionCount: module.attribution.length,
      maplCount: module.mapl.length,
      genCount: module.journal.length,
      summary: module.summary,
    }));

  await writeFile(
    join(outDir, 'showcase-index.json'),
    `${JSON.stringify(index, null, 2)}\n`,
  );

  const collections = built.reduce((acc, module) => {
    acc[module.collection] = (acc[module.collection] ?? 0) + 1;
    return acc;
  }, {});

  process.stdout.write(
    `built ${built.length} module fixtures (${JSON.stringify(collections)}); ` +
      `showcase index lists ${index.length} selfhost modules\n`,
  );
};

run().catch((error) => {
  process.stderr.write(`${error instanceof Error ? error.stack : String(error)}\n`);
  process.exitCode = 1;
});
