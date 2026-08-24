import assert from 'node:assert/strict'
import { execFileSync } from 'node:child_process'
import { existsSync } from 'node:fs'
import { mkdir } from 'node:fs/promises'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

import { chromium } from 'playwright-core'

const baseUrl = process.env.MARKTREE_VISUAL_URL ?? 'http://localhost:5173/'
const output = process.env.MARKTREE_VISUAL_OUTPUT?.trim()
  ? path.resolve(process.env.MARKTREE_VISUAL_OUTPUT.trim())
  : fileURLToPath(new URL('../test-results/', import.meta.url))

function commandPath(command) {
  try {
    const locator = process.platform === 'win32' ? 'where.exe' : 'which'
    return execFileSync(locator, [command], { encoding: 'utf8' })
      .split(/\r?\n/)
      .map((value) => value.trim())
      .find(Boolean)
  } catch {
    return undefined
  }
}

function browserExecutable() {
  const configured = process.env.MARKTREE_BROWSER_PATH?.trim()
  if (configured) {
    if (existsSync(configured)) return configured
    throw new Error(`MARKTREE_BROWSER_PATH does not exist: ${configured}`)
  }
  const playwrightBrowser = chromium.executablePath()
  if (existsSync(playwrightBrowser)) return playwrightBrowser
  for (const command of ['msedge', 'msedge.exe', 'google-chrome', 'chromium']) {
    const resolved = commandPath(command)
    if (resolved && existsSync(resolved)) return resolved
  }
  if (process.platform === 'win32') {
    for (const root of [
      process.env['PROGRAMFILES(X86)'],
      process.env.PROGRAMFILES,
      process.env.LOCALAPPDATA,
    ].filter(Boolean)) {
      for (const relative of [
        ['Microsoft', 'Edge', 'Application', 'msedge.exe'],
        ['Google', 'Chrome', 'Application', 'chrome.exe'],
      ]) {
        const candidate = path.join(root, ...relative)
        if (existsSync(candidate)) return candidate
      }
    }
  }
  throw new Error(
    'No Chromium browser was found. Set MARKTREE_BROWSER_PATH to a stable browser executable.',
  )
}

await mkdir(output, { recursive: true })
const browser = await chromium.launch({ executablePath: browserExecutable(), headless: true })

try {
  for (const scenario of [
    {
      name: 'desktop-light-plain',
      width: 1440,
      height: 900,
      dark: false,
      mobile: false,
      git: false,
    },
    {
      name: 'desktop-dark-git',
      width: 1024,
      height: 700,
      dark: true,
      mobile: false,
      git: true,
    },
    {
      name: 'desktop-min-git',
      width: 980,
      height: 640,
      dark: false,
      mobile: false,
      git: true,
    },
    {
      name: 'mobile-dark-plain',
      width: 390,
      height: 844,
      dark: true,
      mobile: true,
      git: false,
    },
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
    const url = new URL(baseUrl)
    url.searchParams.set('demo', scenario.git ? 'git' : 'plain')
    await page.goto(url.toString(), { waitUntil: 'networkidle' })
    await page.locator('.cm-content').waitFor()
    assert.equal(errors.length, 0, errors.join('\n'))
    assert.ok((await page.locator('.cm-content').innerText()).includes('Marktree'))
    assert.ok(
      (await page.evaluate(() => document.documentElement.scrollWidth)) <= scenario.width + 1,
      `${scenario.name} layout overflows horizontally`,
    )
    const editorScroller = await page.locator('.cm-scroller').boundingBox()
    assert.ok(
      editorScroller && editorScroller.width <= scenario.width + 1,
      `${scenario.name} editor is wider than the visible workspace`,
    )

    if (!scenario.mobile) {
      await page.locator('.window-titlebar').waitFor()
      assert.equal(await page.locator('.window-controls button').count(), 3)
      await page.locator('.workspace-rail').waitFor()
      await page.keyboard.press('Control+P')
      await page.locator('.command-palette-dialog').waitFor()
      assert.ok(await page.locator('.command-palette-results > button').count())
      if (scenario.git) {
        assert.ok(
          (await page.locator('.command-palette-results').innerText()).includes('book'),
          `${scenario.name} command center does not expose worktrees`,
        )
      }
      await page.locator('.command-palette-dialog > label input').fill('Marktree')
      await page.locator('.command-search-filters').waitFor()
      assert.equal(await page.locator('.command-search-filters select').count(), 2)
      await page.screenshot({
        path: path.join(output, `${scenario.name}-commands.png`),
        fullPage: false,
      })
      await page.keyboard.press('Escape')
      assert.equal(await page.locator('.command-palette-dialog').count(), 0)

      await page.keyboard.press('Control+Shift+P')
      await page.locator('.command-palette-dialog > label input').fill('>sync')
      const syncCommandCount = await page
        .locator('.command-palette-results > button')
        .filter({ hasText: /Sync|同步/ })
        .count()
      assert.equal(syncCommandCount, scenario.git ? 1 : 0)
      await page.keyboard.press('Escape')

      const outlineButton = page.locator('.editor-actions button:has(.lucide-list-tree)')
      await outlineButton.click()
      await page.locator('.document-outline').waitFor()
      assert.ok(await page.locator('.document-outline button').count())
      const favoriteButton = page.locator('.editor-actions button:has(.lucide-star)')
      await favoriteButton.click()
      await page.locator('.favorite-file-row').waitFor()
      const readingButton = page.locator('.editor-actions button:has(.lucide-book-open)')
      await readingButton.click()
      await page.locator('.markdown-editor-shell.reading-mode').waitFor()
      await readingButton.click()
      const sidebarSearch = page.locator('.workspace-sidebar .search-box input')
      await sidebarSearch.fill('ideas')
      await page.locator('.sidebar-search-row').waitFor()
      assert.ok(
        (await page.locator('.sidebar-search-row').innerText()).includes('ideas.md'),
        `${scenario.name} does not expose the collapsed-directory search match`,
      )
      await sidebarSearch.fill('')
      await page.locator('button[title="notes"]').click()
      await page.locator('button[title="notes/ideas.md"]').waitFor()
      await page.locator('button[title="notes/ideas.md"]').click()
      await page.locator('.tree-row.active').waitFor()
      if (scenario.git) {
        assert.equal(await page.locator('.enable-git-button').count(), 0)
        await page.locator('.advanced-git-button').click()
        await page.locator('.git-panel').waitFor()
        await page.locator('.advanced-worktrees').waitFor()
        const editorWithGitPanel = await page.locator('.cm-scroller').boundingBox()
        const gitPanel = await page.locator('.git-panel').boundingBox()
        const editorShell = await page.locator('.editor-workspace').boundingBox()
        const gitPanelWidth = await page.locator('.git-panel').evaluate((element) => ({
          client: element.clientWidth,
          scroll: element.scrollWidth,
        }))
        assert.ok(
          editorWithGitPanel && editorWithGitPanel.width >= 300,
          `${scenario.name} Git panel leaves less than 300px for the editor: ${JSON.stringify(editorWithGitPanel)}`,
        )
        assert.ok(
          gitPanel && editorShell && gitPanel.x >= editorShell.x + editorShell.width - 1,
          `${scenario.name} Git panel overlaps the editor`,
        )
        assert.ok(
          gitPanel && gitPanel.y >= 0 && gitPanel.y + gitPanel.height <= scenario.height + 1,
          `${scenario.name} Git panel escapes the viewport: ${JSON.stringify(gitPanel)}`,
        )
        assert.ok(
          gitPanelWidth.scroll <= gitPanelWidth.client + 1,
          `${scenario.name} Git controls overflow horizontally: ${JSON.stringify(gitPanelWidth)}`,
        )
        await page.locator('.commit-box').scrollIntoViewIfNeeded()
        const commitBox = await page.locator('.commit-box').boundingBox()
        assert.ok(
          commitBox && commitBox.y >= gitPanel.y && commitBox.y + commitBox.height <= scenario.height + 1,
          `${scenario.name} commit controls are not reachable: ${JSON.stringify(commitBox)}`,
        )
      } else {
        assert.equal(await page.locator('.enable-git-button').count(), 0)
        assert.equal(await page.locator('.advanced-git-button').count(), 0)
        assert.equal(await page.locator('.sync-button').count(), 0)
        assert.equal(await page.locator('.git-panel').count(), 0)
        await page.locator('.workspace-more-button').click()
        await page.locator('.credentials-dialog .enable-git-button').waitFor()
        await page.locator('.operation-log').waitFor()
        await page.locator('.editor-preferences-panel').scrollIntoViewIfNeeded()
        await page.locator('.editor-preferences-panel').waitFor()
        const settingsBox = await page.locator('.credentials-dialog').boundingBox()
        assert.ok(settingsBox && settingsBox.y >= 0)
        assert.ok(settingsBox.y + settingsBox.height <= scenario.height + 1)
        await page.screenshot({
          path: path.join(output, `${scenario.name}-settings.png`),
          fullPage: false,
        })
        await page.locator('.credentials-dialog > header button').click()
        assert.equal(await page.locator('.credentials-dialog').count(), 0)
      }
    } else {
      assert.equal(await page.locator('.window-titlebar').count(), 0)
      assert.equal(await page.locator('.workspace-rail').count(), 0)
      assert.equal(await page.locator('.mobile-subtitle button').count(), 0)
      assert.equal(await page.locator('.git-panel').count(), 0)
      await page.locator('.markdown-toolbar').waitFor()
      assert.ok(await page.locator('.markdown-toolbar button').count())
      assert.equal(await page.locator('.markdown-toolbar .lucide-table-2').count(), 1)
      assert.equal(await page.locator('.markdown-toolbar .lucide-sigma').count(), 1)
      assert.equal(await page.locator('.markdown-toolbar .lucide-paperclip').count(), 1)
      await page.locator('.workspace-more-button').click()
      await page.locator('.credentials-dialog .enable-git-button').waitFor()
      await page.locator('.operation-log').waitFor()
      await page.locator('.editor-preferences-panel').scrollIntoViewIfNeeded()
      await page.locator('.editor-preferences-panel').waitFor()
      const settingsBox = await page.locator('.credentials-dialog').boundingBox()
      assert.ok(settingsBox && settingsBox.y >= 0)
      assert.ok(settingsBox.y + settingsBox.height <= scenario.height + 1)
      await page.screenshot({
        path: path.join(output, `${scenario.name}-settings.png`),
        fullPage: false,
      })
      await page.locator('.credentials-dialog > header button').click()
    }
    await page.screenshot({
      path: path.join(output, `${scenario.name}.png`),
      fullPage: false,
    })
    await context.close()
  }
} finally {
  await browser.close()
}
