import {afterAll, beforeAll, describe, expect, it} from 'vitest'
import {chromium, type Browser} from 'playwright'
import {E2E_ORIGIN} from './global-setup'

let browser: Browser

beforeAll(async () => {
  browser = await chromium.launch()
})

afterAll(async () => {
  await browser?.close()
})

describe('site smoke', () => {
  it('highlights fenced code blocks in the docs', async () => {
    const page = await browser.newPage()
    await page.goto(`${E2E_ORIGIN}/docs/format`, {waitUntil: 'domcontentloaded'})
    await expect
      .poll(
        () => page.locator('pre .line span, pre code span, .shiki span').count(),
        {timeout: 20_000},
      )
      .toBeGreaterThan(0)
    await page.close()
  })

  it('serves llms-full.txt', async () => {
    const response = await fetch(`${E2E_ORIGIN}/llms-full.txt`)
    expect(response.status).toBe(200)
    const body = await response.text()
    expect(body.length).toBeGreaterThan(100)
    await Promise.resolve()
  })

  it('hydrates a docs NaplExample into a live editor with hover', async () => {
    const page = await browser.newPage({viewport: {width: 1440, height: 1400}})
    await page.goto(`${E2E_ORIGIN}/docs`, {waitUntil: 'domcontentloaded'})
    const mount = page.locator('[data-napl-example="greeting"]')
    await expect.poll(() => mount.count(), {timeout: 30_000}).toBeGreaterThan(0)
    await mount.first().scrollIntoViewIfNeeded()
    await expect
      .poll(() => page.locator('[data-napl-example="greeting"] [data-playground-root]').count(), {
        timeout: 30_000,
      })
      .toBe(1)
    const promptLine = page
      .locator('[data-napl-example="greeting"] .cm-line', {hasText: 'Expose'})
      .first()
    await expect.poll(() => promptLine.count(), {timeout: 15_000}).toBeGreaterThan(0)
    await promptLine.hover()
    await expect
      .poll(() => page.locator('.cm-napl-card, .cm-napl-tooltip').count(), {timeout: 10_000})
      .toBeGreaterThan(0)
    await page.close()
  })

  it('keeps a docs NaplExample bounded within the viewport, scrolling internally', async () => {
    const page = await browser.newPage({viewport: {width: 1280, height: 720}})
    await page.goto(`${E2E_ORIGIN}/docs`, {waitUntil: 'domcontentloaded'})
    const root = page.locator('[data-napl-example="greeting"] [data-playground-root]')
    await expect.poll(() => root.count(), {timeout: 30_000}).toBe(1)
    await root.scrollIntoViewIfNeeded()
    const editor = page.locator('[data-napl-example="greeting"] .cm-editor').first()
    await expect.poll(() => editor.count(), {timeout: 15_000}).toBeGreaterThan(0)
    const height = await editor.evaluate((el) => el.getBoundingClientRect().height)
    expect(height).toBeLessThanOrEqual(720)
    const scroller = page.locator('[data-napl-example="greeting"] .cm-scroller').first()
    const overflowY = await scroller.evaluate((el) => getComputedStyle(el).overflowY)
    expect(['auto', 'scroll']).toContain(overflowY)
    await page.close()
  })
})
