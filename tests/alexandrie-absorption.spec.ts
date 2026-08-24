import { createApp, nextTick } from 'vue'
import { afterEach, beforeAll, describe, expect, it, vi } from 'vitest'

import AndroidShareDialog from '../src/components/AndroidShareDialog.vue'
import EditorPreferencesPanel from '../src/components/EditorPreferencesPanel.vue'
import EditorWorkspace from '../src/components/EditorWorkspace.vue'
import MarkdownEditor from '../src/components/MarkdownEditor.vue'
import WorkspaceOverlays from '../src/components/WorkspaceOverlays.vue'
import { i18n } from '../src/i18n'
import {
  editorPreferences,
  resetEditorPreferences,
} from '../src/lib/editor/preferences'
import {
  importMarkdownSnippets,
  markdownSnippets,
  renderMarkdownSnippet,
  serializeMarkdownSnippets,
} from '../src/lib/editor/snippets'
import { markdownOutline } from '../src/lib/editor/outline'
import {
  favoriteDocumentKeys,
  isFavoriteDocument,
  migrateWorkspacePaths,
  removeWorkspaceUiState,
  restoredWorkspaceSession,
  saveWorkspaceSession,
  toggleFavoriteDocument,
} from '../src/lib/workspaceUiState'
import type {
  EditorTab,
  PendingAndroidShare,
  WorkspaceDescriptor,
  WorkspaceEntry,
} from '../src/types'

const mounted: Array<ReturnType<typeof createApp>> = []

beforeAll(() => {
  Object.defineProperty(globalThis, 'ResizeObserver', {
    configurable: true,
    value: class {
      observe() {}
      unobserve() {}
      disconnect() {}
    },
  })
})

afterEach(() => {
  for (const app of mounted.splice(0)) app.unmount()
  document.body.replaceChildren()
  localStorage.clear()
  markdownSnippets.value = []
  favoriteDocumentKeys.value = []
  removeWorkspaceUiState(['E:\\Notes', 'E:\\Other'])
  resetEditorPreferences()
  vi.restoreAllMocks()
})

function mount(component: Parameters<typeof createApp>[0], props: Record<string, unknown>) {
  const host = document.createElement('div')
  document.body.append(host)
  const app = createApp(component, props)
  app.use(i18n)
  app.mount(host)
  mounted.push(app)
  return host
}

function entry(
  path: string,
  fileKind: WorkspaceEntry['fileKind'] = 'markdown',
): WorkspaceEntry {
  return {
    path,
    name: path.split('/').at(-1) ?? path,
    entryType: 'file',
    fileKind,
    size: 12,
    modifiedMs: 10,
    readOnly: false,
    gitStatus: null,
  }
}

function tab(path: string, content: string): EditorTab {
  return {
    root: 'E:\\Notes',
    path,
    title: path.split('/').at(-1) ?? path,
    content,
    diskContent: content,
    modifiedMs: 10,
    sha256: 'hash',
    readOnly: false,
    encoding: 'utf8',
    lineEnding: 'lf',
    revision: 0,
    savedRevision: 0,
    dirty: false,
    saving: false,
  }
}

describe('absorbed Markdown authoring capabilities', () => {
  it('inserts table, formula, and existing-file links through the real CodeMirror buffer', async () => {
    const tableUpdate = vi.fn()
    const tableHost = mount(MarkdownEditor, {
      modelValue: '# Start\n',
      root: 'E:\\Notes',
      path: 'notes/current.md',
      markdown: true,
      linkCandidates: [
        entry('notes/current.md'),
        entry('assets/diagram one.png', 'image'),
      ],
      loadWorkspaceImage: async () => '',
      'onUpdate:modelValue': tableUpdate,
    })
    await nextTick()

    tableHost
      .querySelector<HTMLButtonElement>(`button[title="${i18n.global.t('app.table')}"]`)!
      .click()
    expect(tableUpdate).toHaveBeenLastCalledWith(
      '| Column 1 | Column 2 |\n| --- | --- |\n| Value |  |# Start\n',
    )

    const formulaUpdate = vi.fn()
    const formulaHost = mount(MarkdownEditor, {
      modelValue: '',
      root: 'E:\\Notes',
      path: 'notes/current.md',
      markdown: true,
      linkCandidates: [entry('assets/diagram one.png', 'image')],
      loadWorkspaceImage: async () => '',
      'onUpdate:modelValue': formulaUpdate,
    })
    await nextTick()
    formulaHost
      .querySelector<HTMLButtonElement>(`button[title="${i18n.global.t('app.formula')}"]`)!
      .click()
    expect(formulaUpdate).toHaveBeenLastCalledWith('$formula$')

    const linkUpdate = vi.fn()
    const linkHost = mount(MarkdownEditor, {
      modelValue: '',
      root: 'E:\\Notes',
      path: 'notes/current.md',
      markdown: true,
      linkCandidates: [entry('assets/diagram one.png', 'image')],
      loadWorkspaceImage: async () => '',
      'onUpdate:modelValue': linkUpdate,
    })
    await nextTick()
    linkHost
      .querySelector<HTMLButtonElement>(
        `button[title="${i18n.global.t('app.chooseExistingFile')}"]`,
      )!
      .click()
    await nextTick()
    linkHost.querySelector<HTMLButtonElement>('.workspace-link-results button')!.click()
    expect(linkUpdate).toHaveBeenLastCalledWith(
      '![diagram one](<../assets/diagram one.png>)',
    )
  })

  it('round-trips versioned personal snippets and keeps defaults standard Markdown', () => {
    markdownSnippets.value = [
      { id: 'one', name: 'Callout', shortcut: 'Call Out', body: '> {{selection}}{{cursor}}' },
    ]
    const serialized = serializeMarkdownSnippets()
    markdownSnippets.value = []
    expect(importMarkdownSnippets(serialized, true)).toBe(1)
    expect(markdownSnippets.value).toEqual([
      { id: 'one', name: 'Callout', shortcut: 'call-out', body: '> {{selection}}{{cursor}}' },
    ])
    expect(serialized).not.toContain(':::')
    expect(renderMarkdownSnippet('**{{selection}}**{{cursor}}', 'source')).toEqual({
      text: '**source**',
      cursor: 10,
    })
  })

  it('extracts a real hierarchical Markdown outline without headings from code fences', () => {
    expect(
      markdownOutline('# One\n\n```md\n# Not a heading\n```\n\nTwo\n---\n\n### Three ###\n'),
    ).toEqual([
      { text: 'One', level: 1, position: 0 },
      { text: 'Two', level: 2, position: 34 },
      { text: 'Three', level: 3, position: 43 },
    ])
  })
})

describe('path-only document continuity', () => {
  it('restores tabs and migrates favorites by paths without persisting document content', () => {
    saveWorkspaceSession('E:\\Notes', ['drafts/a.md', 'drafts/b.md'], 'drafts/b.md')
    toggleFavoriteDocument('E:\\Notes', 'drafts/b.md')
    migrateWorkspacePaths('E:\\Notes', [
      { oldPath: 'drafts/a.md', newPath: 'archive/a.md' },
      { oldPath: 'drafts/b.md', newPath: 'archive/b.md' },
    ])

    expect(restoredWorkspaceSession('E:\\Notes')).toEqual({
      tabs: ['archive/a.md', 'archive/b.md'],
      active: 'archive/b.md',
      expanded: [],
    })
    expect(isFavoriteDocument('E:\\Notes', 'archive/b.md')).toBe(true)
    expect(localStorage.getItem('marktree-workspace-ui-v1')).not.toContain('secret document body')
  })

  it('persists bounded editor preferences synchronously from the visible settings panel', async () => {
    const host = mount(EditorPreferencesPanel, {})
    await nextTick()
    const selects = host.querySelectorAll<HTMLSelectElement>('select')
    selects[0]!.value = 'monospace'
    selects[0]!.dispatchEvent(new Event('change'))
    await nextTick()

    expect(editorPreferences.fontFamily).toBe('monospace')
    expect(JSON.parse(localStorage.getItem('marktree-editor-preferences-v1') ?? '{}')).toMatchObject({
      fontFamily: 'monospace',
      fontSize: 16,
      sidebarWidth: 280,
    })
  })
})

describe('navigation, command search, previews, and Android intake', () => {
  it('exposes outline, breadcrumb, previous/next, favorite, reading, and print actions', async () => {
    const active = tab('notes/current.md', '# Current\n\nBody')
    const openPath = vi.fn()
    const revealPath = vi.fn()
    const favorite = vi.fn()
    const print = vi.spyOn(window, 'print').mockImplementation(() => undefined)
    const host = mount(EditorWorkspace, {
      tabs: [active],
      activeTab: active,
      activeKey: `${active.root}\n${active.path}`,
      entries: [entry('a.md'), entry('notes/current.md'), entry('z.md')],
      favorite: false,
      dark: false,
      loadWorkspaceImage: async () => '',
      onOpenPath: openPath,
      onRevealPath: revealPath,
      onToggleFavorite: favorite,
    })
    await nextTick()

    const locationButtons = host.querySelectorAll<HTMLButtonElement>('.document-breadcrumbs button')
    locationButtons[0]!.click()
    expect(revealPath).toHaveBeenCalledWith('notes')

    const actionButtons = [...host.querySelectorAll<HTMLButtonElement>('.editor-actions button')]
    actionButtons
      .find(
        (button) => button.ariaLabel === i18n.global.t('app.previousDocumentAction'),
      )!
      .click()
    actionButtons
      .find((button) => button.ariaLabel === i18n.global.t('app.nextDocumentAction'))!
      .click()
    expect(openPath.mock.calls).toEqual([['a.md'], ['z.md']])

    actionButtons.find((button) => button.querySelector('.lucide-star'))!.click()
    expect(favorite).toHaveBeenCalledOnce()
    actionButtons
      .find((button) => button.textContent?.includes(i18n.global.t('app.outline')))!
      .click()
    await nextTick()
    expect(host.querySelector('.document-outline')?.textContent).toContain('Current')

    actionButtons
      .find((button) => button.textContent?.includes(i18n.global.t('app.readingView')))!
      .click()
    await nextTick()
    expect(host.querySelector('.markdown-editor-shell')?.classList.contains('reading-mode')).toBe(true)
    actionButtons
      .find((button) => button.textContent?.includes(i18n.global.t('app.print')))!
      .click()
    await nextTick()
    expect(print).toHaveBeenCalledOnce()
    expect(host.querySelector('.document-stats')?.textContent).toContain('3')
  })

  it('shows search filters, highlights context, and renders each supported preview surface', async () => {
    const host = mount(WorkspaceOverlays, {
      commandPaletteOpen: true,
      commandPaletteQuery: 'needle',
      commandPaletteResults: [
        {
          key: 'document:one',
          kind: 'document',
          title: 'note.md',
          detail: 'notes/note.md:4 · before needle after',
          result: {
            worktree: 'Notes',
            root: 'E:\\Notes',
            path: 'notes/note.md',
            line: 4,
            column: 8,
            snippet: 'before needle after',
            matchType: 'content',
            fileKind: 'markdown',
            modifiedMs: 10,
          },
        },
      ],
      commandPaletteSearching: false,
      commandPalettePathPrefix: 'notes',
      commandPaletteFileKind: 'markdown',
      commandPaletteModifiedDays: 7,
    })
    await nextTick()
    expect(host.querySelectorAll('.command-search-filters select')).toHaveLength(2)
    expect(host.querySelector('mark')?.textContent).toBe('needle')

    for (const [kind, selector] of [
      ['image', 'img'],
      ['pdf', 'iframe'],
      ['audio', 'audio'],
      ['video', 'video'],
    ] as const) {
      const previewHost = mount(WorkspaceOverlays, {
        commandPaletteOpen: false,
        commandPaletteQuery: '',
        commandPaletteResults: [],
        commandPalettePathPrefix: '',
        commandPaletteFileKind: 'all',
        commandPaletteModifiedDays: 0,
        filePreview: {
          root: 'E:\\Notes',
          path: `media/file.${kind}`,
          url: 'data:application/octet-stream;base64,AA==',
          kind,
          mediaType: 'application/octet-stream',
        },
      })
      await nextTick()
      expect(previewHost.querySelector(`.file-preview-dialog ${selector}`)).not.toBeNull()
    }
  })

  it('lets Android users choose the real workspace, target directory, and current document', async () => {
    const share: PendingAndroidShare = {
      text: 'shared text',
      subject: 'Subject',
      filePath: null,
      fileName: null,
      mediaType: 'text/plain',
      kind: 'text',
    }
    const workspace: WorkspaceDescriptor = {
      id: 'notes',
      name: 'Notes',
      root: 'E:\\Notes',
      git: null,
    }
    const imported = vi.fn()
    const selectedDirectory = vi.fn()
    const host = mount(AndroidShareDialog, {
      share,
      workspaces: [workspace],
      selectedRoot: workspace.root,
      selectedDirectory: '',
      directories: ['inbox'],
      activeRoot: workspace.root,
      activeDocumentPath: 'notes/current.md',
      onImport: imported,
      onSelectDirectory: selectedDirectory,
    })
    await nextTick()

    const selects = host.querySelectorAll<HTMLSelectElement>('select')
    selects[1]!.value = 'inbox'
    selects[1]!.dispatchEvent(new Event('change'))
    expect(selectedDirectory).toHaveBeenCalledWith('inbox')
    const checkbox = host.querySelector<HTMLInputElement>('input[type="checkbox"]')!
    checkbox.click()
    await nextTick()
    host.querySelector<HTMLButtonElement>('footer .primary')!.click()
    expect(imported).toHaveBeenCalledWith('notes/current.md')
  })
})
