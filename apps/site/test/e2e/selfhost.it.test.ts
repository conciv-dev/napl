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

const openSelfhost = async (page: Page): Promise<void> => {
  await page.goto(`${E2E_ORIGIN}/selfhost`, {waitUntil: 'domcontentloaded'})
  await expect.poll(() => page.getByTestId('showcase-detail').count(), {timeout: 30_000}).toBe(1)
  await expect
    .poll(() => page.getByTestId('module-name').textContent(), {timeout: 30_000})
    .not.toBe('')
}

describe('/selfhost workbench interactions', () => {
  it('expands and collapses a tree folder', async () => {
    const page = await browser.newPage({viewport: {width: 1440, height: 900}})
    await openSelfhost(page)
    const file = page.getByTestId('tree-file-scanner')
    await expect.poll(() => file.isVisible(), {timeout: 15_000}).toBe(true)
    await page.getByTestId('tree-folder-core').click()
    await expect.poll(() => file.isVisible()).toBe(false)
    await page.getByTestId('tree-folder-core').click()
    await expect.poll(() => file.isVisible()).toBe(true)
    await page.close()
  })

  it('switches the editor content when a file is clicked', async () => {
    const page = await browser.newPage({viewport: {width: 1440, height: 900}})
    await openSelfhost(page)
    await page.getByTestId('tree-file-gen_prompt_diff').click()
    await expect
      .poll(() => page.getByTestId('module-name').textContent(), {timeout: 15_000})
      .toBe('gen_prompt_diff')
    await expect
      .poll(
        () => page.getByTestId('prompt-pane').locator('.cm-content').textContent(),
        {timeout: 15_000},
      )
      .toContain('gen_prompt_diff')
    await page.close()
  })

  it('resizes panels by dragging the divider', async () => {
    const page = await browser.newPage({viewport: {width: 1440, height: 900}})
    await openSelfhost(page)
    const sidebar = page.getByTestId('sidebar')
    const before = await sidebar.boundingBox()
    const handle = page.locator('[role="separator"]').first()
    const box = await handle.boundingBox()
    expect(box).not.toBeNull()
    const midX = box!.x + box!.width / 2
    const midY = box!.y + box!.height / 2
    await page.mouse.move(midX, midY)
    await page.mouse.down()
    await page.mouse.move(midX + 160, midY, {steps: 12})
    await page.mouse.up()
    await expect
      .poll(async () => (await sidebar.boundingBox())?.width ?? 0, {timeout: 10_000})
      .toBeGreaterThan((before?.width ?? 0) + 40)
    await page.close()
  })

  it('runs the replay and lands generated code in the UI', async () => {
    const page = await browser.newPage({viewport: {width: 1440, height: 900}})
    await openSelfhost(page)
    await page.getByTestId('tree-file-gen_prompt_diff').click()
    await expect
      .poll(() => page.getByTestId('module-name').textContent(), {timeout: 15_000})
      .toBe('gen_prompt_diff')
    const replay = page.getByTestId('replay-gen')
    await expect.poll(() => replay.isEnabled(), {timeout: 15_000}).toBe(true)
    await replay.click()
    await expect
      .poll(
        () => page.getByTestId('generated-pane').locator('.cm-content').textContent(),
        {timeout: 20_000},
      )
      .toContain('compute_prompt_diff')
    await page.close()
  })

  it('shows an interactive attribution card with jump when hovering a generated line', async () => {
    const page = await browser.newPage({viewport: {width: 1440, height: 900}})
    await openSelfhost(page)
    await page.getByTestId('tree-file-gen_prompt_diff').click()
    const genLine = page
      .getByTestId('generated-pane')
      .locator('.cm-line', {hasText: 'compute_prompt_diff'})
      .first()
    await expect.poll(() => genLine.count(), {timeout: 15_000}).toBeGreaterThan(0)
    await genLine.hover()
    const card = page.locator('.cm-napl-card')
    await expect.poll(() => card.count(), {timeout: 10_000}).toBeGreaterThan(0)
    await expect.poll(() => card.first().textContent()).toContain('prompt')
    const jump = card.locator('.cm-napl-card__jump')
    await expect.poll(() => jump.count()).toBeGreaterThan(0)
    await jump.first().click()
    await expect
      .poll(() => page.getByTestId('prompt-pane').locator('.cm-napl-linked').count(), {
        timeout: 10_000,
      })
      .toBeGreaterThan(0)
    await page.close()
  })

  it('never scrolls the page horizontally at 1440 and 900', async () => {
    for (const width of [1440, 900]) {
      const page = await browser.newPage({viewport: {width, height: 860}})
      await openSelfhost(page)
      const overflow = await page.evaluate(() => {
        const el = document.scrollingElement ?? document.documentElement
        return el.scrollWidth - el.clientWidth
      })
      expect(overflow).toBeLessThanOrEqual(2)
      await page.close()
    }
  })

  it('renders editor tokens in both themes', async () => {
    for (const [theme, expectDark] of [
      ['dark', true],
      ['light', false],
    ] as const) {
      const page = await browser.newPage({viewport: {width: 1440, height: 900}})
      await page.addInitScript((value) => {
        window.localStorage.setItem('theme', value)
      }, theme)
      await openSelfhost(page)
      const isDark = await page.evaluate(() => document.documentElement.classList.contains('dark'))
      expect(isDark).toBe(expectDark)
      const coloredTokens = await page.evaluate(() => {
        const spans = Array.from(document.querySelectorAll('.cm-content .cm-line span'))
        return spans.filter((span) => {
          const color = getComputedStyle(span).color
          return color && color !== 'rgb(0, 0, 0)'
        }).length
      })
      expect(coloredTokens).toBeGreaterThan(0)
      await page.close()
    }
  })
})
