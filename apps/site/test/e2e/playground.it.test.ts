import {afterAll, beforeAll, describe, expect, it} from 'vitest'
import {chromium, type Browser, type Page} from 'playwright'
import {E2E_ORIGIN} from './global-setup'

let browser: Browser

beforeAll(async () => {
  browser = await chromium.launch()
})

afterAll(async () => {
  await browser?.close()
})

const openPlayground = async (page: Page): Promise<void> => {
  await page.goto(`${E2E_ORIGIN}/docs/thinking`, {waitUntil: 'domcontentloaded'})
  const mount = page.locator('[data-playground-mount="gen_prompt_diff"]')
  await expect.poll(() => mount.count(), {timeout: 30_000}).toBe(1)
  await mount.scrollIntoViewIfNeeded()
  await expect
    .poll(() => page.locator('[data-playground-root="gen_prompt_diff"]').count(), {timeout: 30_000})
    .toBe(1)
}

describe('/docs/thinking', () => {
  it('switches tabs and highlights per-language tokens', async () => {
    const page = await browser.newPage({viewport: {width: 1440, height: 1200}})
    await openPlayground(page)
    await page.locator('.napl-playground__tab', {hasText: 'lib.rs'}).click()
    await expect
      .poll(() => page.locator('.napl-editor .cm-content').first().textContent(), {timeout: 15_000})
      .toContain('compute_prompt_diff')
    const coloredTokens = await page.evaluate(() => {
      const spans = Array.from(document.querySelectorAll('.napl-editor .cm-content .cm-line span'))
      return spans.filter((span) => {
        const color = getComputedStyle(span).color
        return color && color !== 'rgb(0, 0, 0)'
      }).length
    })
    expect(coloredTokens).toBeGreaterThan(0)
    await page.close()
  })

  it('surfaces a frontmatter diagnostic when the prompt is broken', async () => {
    const page = await browser.newPage({viewport: {width: 1440, height: 1200}})
    await openPlayground(page)
    const editor = page.locator('.napl-editor .cm-content').first()
    await editor.click()
    await page.keyboard.press('ControlOrMeta+A')
    await page.keyboard.type('no frontmatter here')
    await expect
      .poll(
        () =>
          page
            .locator('.cm-lintRange-error, .cm-lint-marker-error, .cm-diagnostic-error, .cm-lintRange')
            .count(),
        {timeout: 10_000},
      )
      .toBeGreaterThan(0)
    await page.close()
  })

  it('runs the replay gen and fills the generated tab', async () => {
    const page = await browser.newPage({viewport: {width: 1440, height: 1200}})
    await openPlayground(page)
    const gen = page.locator('.napl-playground__gen')
    await expect.poll(() => gen.isEnabled(), {timeout: 15_000}).toBe(true)
    await gen.click()
    await page.locator('.napl-playground__tab', {hasText: 'lib.rs'}).click()
    await expect
      .poll(() => page.locator('.napl-editor .cm-content').first().textContent(), {timeout: 20_000})
      .toContain('compute_prompt_diff')
    await page.close()
  })
})
