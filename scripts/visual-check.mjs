import assert from 'node:assert/strict'
import { mkdir } from 'node:fs/promises'

import { chromium } from 'playwright-core'

const edge = 'C:\\Program Files (x86)\\Microsoft\\Edge\\Application\\msedge.exe'
const baseUrl = process.env.MARKTREE_VISUAL_URL ?? 'http://localhost:5173/?demo=1'
const output = new URL('../test-results/', import.meta.url)

await mkdir(output, { recursive: true })
const browser = await chromium.launch({ executablePath: edge, headless: true })

try {
  for (const scenario of [
    { name: 'desktop-light', width: 1440, height: 900, dark: false, mobile: false },
    { name: 'desktop-dark-compact', width: 1024, height: 700, dark: true, mobile: false },
    { name: 'mobile-dark', width: 390, height: 844, dark: true, mobile: true },
  ]) {
    const context = await browser.newContext({
      viewport: { width: scenario.width, height: scenario.height },
      deviceScaleFactor: 1,
    })
    await context.addInitScript((dark) => {
      localStorage.setItem('marktree-theme', dark ? 'dark' : 'light')
    }, scenario.dark)
    const page = await context.newPage()
    const errors = []
    page.on('console', (message) => {
      if (message.type() === 'error') errors.push(message.text())
    })
    page.on('pageerror', (error) => errors.push(error.message))
    await page.goto(baseUrl, { waitUntil: 'networkidle' })
    await page.locator('.cm-content').waitFor()
    assert.equal(errors.length, 0, errors.join('\n'))
    assert.ok((await page.locator('.cm-content').innerText()).includes('Marktree'))
    assert.ok(
      (await page.evaluate(() => document.documentElement.scrollWidth)) <= scenario.width + 1,
      `${scenario.name} layout overflows horizontally`,
    )

    if (!scenario.mobile) {
      await page.locator('.repository-rail').waitFor()
      await page.locator('.worktree-section').waitFor()
    } else {
      assert.equal(await page.locator('.repository-rail').count(), 0)
      await page.locator('.mobile-subtitle button').waitFor()
      assert.equal(await page.locator('.worktree-section').count(), 0)
    }
    await page.screenshot({
      path: new URL(`${scenario.name}.png`, output).pathname.slice(1),
      fullPage: false,
    })
    await context.close()
  }
} finally {
  await browser.close()
}
