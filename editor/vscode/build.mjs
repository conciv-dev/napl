import { build } from 'esbuild';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(here, '..', '..');

const shared = {
  bundle: true,
  platform: 'node',
  target: 'node20',
  format: 'cjs',
  sourcemap: false,
  logLevel: 'info',
};

await build({
  ...shared,
  entryPoints: [resolve(here, 'src', 'extension.ts')],
  outfile: resolve(here, 'dist', 'extension.js'),
  external: ['vscode'],
});

await build({
  ...shared,
  entryPoints: [resolve(repoRoot, 'src', 'lsp', 'server.ts')],
  outfile: resolve(here, 'dist', 'server.js'),
  external: [],
});

console.log('extension + server bundled to dist/');
