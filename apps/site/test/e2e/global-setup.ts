import {spawn, type ChildProcess} from 'node:child_process'
import {fileURLToPath} from 'node:url'
import {dirname, resolve} from 'node:path'

export const E2E_PORT = 4788
export const E2E_ORIGIN = `http://127.0.0.1:${E2E_PORT}`

const siteRoot = resolve(dirname(fileURLToPath(import.meta.url)), '../..')

const waitForReady = async (url: string, timeoutMs: number): Promise<void> => {
  const deadline = Date.now() + timeoutMs
  while (Date.now() < deadline) {
    try {
      const response = await fetch(url, {redirect: 'manual'})
      if (response.status < 500) return
    } catch {
      // server not up yet
    }
    await new Promise((r) => setTimeout(r, 500))
  }
  throw new Error(`dev server did not become ready at ${url}`)
}

export default async function setup(): Promise<() => Promise<void>> {
  const server: ChildProcess = spawn(
    'pnpm',
    ['exec', 'vite', 'dev', '--port', String(E2E_PORT), '--host', '127.0.0.1'],
    {cwd: siteRoot, detached: true, stdio: 'ignore'},
  )
  await waitForReady(`${E2E_ORIGIN}/`, 90_000)
  return async () => {
    if (server.pid) {
      try {
        process.kill(-server.pid, 'SIGTERM')
      } catch {
        server.kill('SIGTERM')
      }
    }
  }
}
