import { readFile } from 'node:fs/promises';
import { readFileSync, readdirSync } from 'node:fs';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { beforeAll, describe, expect, it } from 'vitest';
import {
  attributionAtFileLine,
  attributionAtPromptLine,
  bodyLineMap,
  blameReplay,
  driftDetect,
  initNaplWasm,
  maplParse,
  parseFrontmatterDiagnostics,
  scanDocument,
  type BlameLine,
} from '../index.js';

const repoFile = (path: string): string =>
  fileURLToPath(new URL(`../../../${path}`, import.meta.url));

const read = (path: string): Promise<string> => readFile(repoFile(path), 'utf8');

const findPrompt = (dir: string, moduleName: string): string | null => {
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    if (entry.name === 'node_modules' || entry.name === 'target' || entry.name === '.napl') {
      continue;
    }
    const full = join(dir, entry.name);
    if (entry.isDirectory()) {
      const nested = findPrompt(full, moduleName);
      if (nested) {
        return nested;
      }
      continue;
    }
    if (entry.name.endsWith('.napl') || entry.name.endsWith('.🧑')) {
      if (new RegExp(`^module:\\s*${moduleName}\\s*$`, 'm').test(readFileSync(full, 'utf8'))) {
        return full;
      }
    }
  }
  return null;
};

const readPrompt = (moduleName: string): Promise<string> => {
  const found = findPrompt(repoFile('selfhost'), moduleName);
  if (!found) {
    throw new Error(`could not locate the ${moduleName} prompt under selfhost/`);
  }
  return readFile(found, 'utf8');
};

beforeAll(async () => {
  await initNaplWasm();
});

describe('@napl/wasm bindings over real selfhost fixtures', () => {
  it('reports no frontmatter diagnostics for a valid prompt', async () => {
    const content = await readPrompt('body_lines');
    expect(parseFrontmatterDiagnostics(content)).toEqual([]);
  });

  it('surfaces an error diagnostic for a document without frontmatter', () => {
    const diagnostics = parseFrontmatterDiagnostics('no frontmatter here');
    expect(diagnostics).toHaveLength(1);
    expect(diagnostics[0].severity).toBe('error');
  });

  it('scans module value and regions from a real prompt', async () => {
    const content = await readPrompt('body_lines');
    const scan = scanDocument(content);
    expect(scan.frontmatter.present).toBe(true);
    expect(scan.body.present).toBe(true);
    expect(scan.module_value?.value).toBe('body_lines');
  });

  it('maps document body lines', async () => {
    const content = await readPrompt('body_lines');
    const map = bodyLineMap(content);
    expect(map.body_start_line).toBeGreaterThan(0);
    expect(map.lines.length).toBeGreaterThan(0);
  });

  it('parses real .mapl documents with the kind-to-severity invariant', async () => {
    const severityForKind: Record<string, string> = {
      ambiguity: 'error',
      assumption: 'warning',
      'no-op': 'warning',
      note: 'info',
    };
    const modules = ['body_lines', 'schemas_journal', 'reverse', 'targets'];
    const seenKinds = new Set<string>();
    for (const moduleName of modules) {
      const entries = maplParse(await read(`selfhost/.napl/mapl/${moduleName}.mapl`));
      expect(entries.length).toBeGreaterThan(0);
      for (const entry of entries) {
        seenKinds.add(entry.kind);
        expect(entry.severity).toBe(severityForKind[entry.kind]);
        expect(entry.prompt_lines.start).toBeGreaterThanOrEqual(1);
      }
    }
    expect(seenKinds.size).toBeGreaterThan(1);
  });

  it('looks up attribution in both directions', async () => {
    const attribution = await read('selfhost/.napl/attribution/blame_render.yaml');
    const forward = attributionAtPromptLine(attribution, 38);
    expect(forward.length).toBeGreaterThan(0);
    expect(forward[0].file).toBe('blame_render/src/lib.rs');

    const reverse = attributionAtFileLine(
      attribution,
      forward[0].file,
      forward[0].lines.start,
    );
    expect(reverse.some((entry) => entry.prompt_lines.start === 38)).toBe(true);
  });

  it('replays blame from a real journal', async () => {
    const journal = await read('selfhost/.napl/journal.jsonl');
    const path = '.napl/src/rust/schemas_journal/src/lib.rs';
    const lines = blameReplay(journal, path) as BlameLine[];
    expect(Array.isArray(lines)).toBe(true);
    expect(lines.length).toBeGreaterThan(0);
    expect(lines.every((line) => Number.isInteger(line.gen))).toBe(true);

    const single = blameReplay(journal, path, 1) as BlameLine | null;
    expect(single?.line).toBe(1);
  });

  it('detects drift against recorded map hashes', async () => {
    const mapJson = await read('selfhost/.napl/map.json');
    const cleanPath = '.napl/src/rust/extensions/Cargo.toml';
    const cleanContent = await read(`selfhost/${cleanPath}`);
    const contents = {
      [cleanPath]: cleanContent,
      '.napl/src/rust/extensions/src/lib.rs': 'edited by hand\n',
    };
    const drift = driftDetect(mapJson, JSON.stringify(contents));
    const clean = drift.find((file) => file.file === cleanPath);
    const edited = drift.find(
      (file) => file.file === '.napl/src/rust/extensions/src/lib.rs',
    );
    const missing = drift.find((file) => file.status === 'missing');
    expect(clean?.status).toBe('clean');
    expect(edited?.status).toBe('edited');
    expect(missing).toBeDefined();
  });
});
