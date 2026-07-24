import { build } from 'esbuild';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));

await build({
  bundle: true,
  platform: 'node',
  target: 'node20',
  format: 'cjs',
  sourcemap: false,
  logLevel: 'info',
  entryPoints: [resolve(here, 'src', 'extension.ts')],
  outfile: resolve(here, 'dist', 'extension.js'),
  external: ['vscode'],
});

console.log('extension bundled to dist/');
