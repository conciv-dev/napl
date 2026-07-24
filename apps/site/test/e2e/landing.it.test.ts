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

describe('landing hero', () => {
  it('mounts the interactive hero playground and keeps the page clickable', async () => {
    const page = await browser.newPage({viewport: {width: 1440, height: 1000}})
    await page.goto(E2E_ORIGIN, {waitUntil: 'domcontentloaded'})
    await expect
      .poll(() => page.locator('[data-playground-root="greeting"]').count(), {timeout: 30_000})
      .toBe(1)
    const gen = page.locator('.napl-playground__gen')
    await expect.poll(() => gen.isEnabled(), {timeout: 15_000}).toBe(true)
    await gen.click()
    await expect
      .poll(() => page.getByRole('link', {name: /Read the docs/i}).isVisible())
      .toBe(true)
    await page.close()
  })

  it('renders with reduced motion enabled', async () => {
    const context = await browser.newContext({
      viewport: {width: 1440, height: 1000},
      reducedMotion: 'reduce',
    })
    const page = await context.newPage()
    await page.goto(E2E_ORIGIN, {waitUntil: 'domcontentloaded'})
    await expect
      .poll(() => page.getByRole('heading', {name: /prompt/i}).first().isVisible(), {timeout: 20_000})
      .toBe(true)
    await context.close()
  })
})
