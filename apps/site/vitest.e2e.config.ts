import {defineConfig} from 'vitest/config'

export default defineConfig({
  test: {
    environment: 'node',
    include: ['test/e2e/**/*.it.test.ts'],
    globalSetup: ['./test/e2e/global-setup.ts'],
    testTimeout: 60_000,
    hookTimeout: 120_000,
    fileParallelism: false,
    pool: 'forks',
  },
})
