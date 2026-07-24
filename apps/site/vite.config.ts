import react from '@vitejs/plugin-react'
import {tanstackStart} from '@tanstack/react-start/plugin/vite'
import {cloudflare} from '@cloudflare/vite-plugin'
import {defineConfig, type Plugin} from 'vite'
import tailwindcss from '@tailwindcss/vite'
import mdx from 'fumadocs-mdx/vite'

function dropWasmFromServerBundle(): Plugin {
  return {
    name: 'drop-wasm-from-server-bundle',
    generateBundle(_options, bundle) {
      if (this.environment.name !== 'ssr') return
      Object.keys(bundle)
        .filter((fileName) => fileName.endsWith('.wasm'))
        .forEach((fileName) => delete bundle[fileName])
    },
  }
}

export default defineConfig({
  server: {
    port: 3002,
  },
  plugins: [
    dropWasmFromServerBundle(),
    cloudflare({viteEnvironment: {name: 'ssr'}}),
    mdx(),
    tailwindcss(),
    tanstackStart({
      prerender: {
        enabled: true,
      },
    }),
    react(),
  ],
  resolve: {
    tsconfigPaths: true,
  },
})
