import { createApp, defineComponent, h, nextTick, ref } from 'vue'
import { afterEach, describe, expect, it, vi } from 'vitest'

import ConflictDialog from '../src/components/ConflictDialog.vue'
import GitPanel from '../src/components/GitPanel.vue'
import MarkdownEditor from '../src/components/MarkdownEditor.vue'
import SnippetManager from '../src/components/SnippetManager.vue'
import ThreeWayConflictResolver from '../src/components/ThreeWayConflictResolver.vue'
import WorkspaceDialogs from '../src/components/WorkspaceDialogs.vue'
import WorkspaceSidebar from '../src/components/WorkspaceSidebar.vue'
import { i18n } from '../src/i18n'
import { markdownSnippets } from '../src/lib/editor/snippets'
import type {
  ConflictRecord,
  OperationLogEntry,
  WorkspaceDescriptor,
  WorkspaceEntry,
} from '../src/types'

const mounted: Array<ReturnType<typeof createApp>> = []

afterEach(() => {
  for (const app of mounted.splice(0)) app.unmount()
  document.body.replaceChildren()
  localStorage.clear()
  markdownSnippets.value = []
})

describe('user-visible component interactions', () => {
  it('shows native operation records in workspace settings without exposing content', async () => {
    const entry: OperationLogEntry = {
      timestamp: '2026-08-01T08:00:00Z',
      category: 'workspace',
      action: 'writeFile',
      phase: 'filesystemApplied',
      outcome: 'succeeded',
      root: 'E:\\Notes',
      operationId: 'operation-1',
      errorCode: null,
    }
    const host = document.createElement('div')
    document.body.append(host)
    const app = createApp(WorkspaceDialogs, {
      modal: 'settings',
      form: {
        remoteUrl: '',
        destination: '',
        documentPath: '',
        worktreeName: '',
        worktreePath: '',
        worktreeBranch: '',
        worktreeStart: 'HEAD',
        credentialUsername: '',
        credentialToken: '',
        assetsDir: 'assets',
        ignoreRules: '',
        workspaceName: '',
      },
      nativeAndroid: false,
      gitEnabled: false,
      githubPending: false,
      trashEntries: [],
      operationLog: [entry],
    })
    app.use(i18n)
    app.mount(host)
    mounted.push(app)
    await nextTick()

    const details = host.querySelector<HTMLDetailsElement>('.operation-log')!
    details.open = true
    await nextTick()

    expect(details.textContent).toContain(i18n.global.t('app.operationAction.writeFile'))
    expect(details.textContent).toContain(i18n.global.t('app.operationOutcome.succeeded'))
    expect(details.textContent).toContain('E:\\Notes')
    expect(details.textContent).not.toContain('document body')
  })

  it('describes binary conflict versions without calling them deleted', async () => {
    const resolved = vi.fn()
    const conflict: ConflictRecord = {
      path: 'assets/shared.png',
      kind: 'binary',
      ancestor: null,
      local: null,
      remote: null,
      ancestorExists: true,
      localExists: true,
      remoteExists: true,
      recoveryId: '1234567890abcdef1234',
      choice: null,
    }
    const host = document.createElement('div')
    document.body.append(host)
    const app = createApp(ConflictDialog, {
      conflicts: [conflict],
      nativeAndroid: false,
      syncing: false,
      onResolveChoice: resolved,
    })
    app.use(i18n)
    app.mount(host)
    mounted.push(app)
    await nextTick()

    const versions = [...host.querySelectorAll('.comparison-grid pre')]
    const binaryLabel = i18n.global.t('app.binaryConflictVersion')
    const deletedLabel = i18n.global.t('app.deletedVersion')
    expect(versions).toHaveLength(2)
    expect(versions.every((version) => version.textContent === binaryLabel)).toBe(true)
    expect(versions.every((version) => version.textContent !== deletedLabel)).toBe(true)

    host.querySelector<HTMLButtonElement>('.conflict-dialog footer .primary')!.click()
    expect(resolved).toHaveBeenCalledWith(conflict, 'local')
  })

  it('lets a user choose one conflict segment and emits the visible final document', async () => {
    const resolved = vi.fn()
    const conflict: ConflictRecord = {
      path: 'notes/shared.md',
      kind: 'text',
      ancestor: '# Shared\n\nBase\n',
      local: '# Shared\n\nLocal\n',
      remote: '# Shared\n\nRemote\n',
      ancestorExists: true,
      localExists: true,
      remoteExists: true,
      recoveryId: '1234567890abcdef1234',
      choice: null,
    }
    const host = document.createElement('div')
    document.body.append(host)
    const app = createApp(ThreeWayConflictResolver, {
      conflict,
      onResolve: resolved,
    })
    app.use(i18n)
    app.mount(host)
    mounted.push(app)
    await nextTick()

    const candidateButtons = [...host.querySelectorAll<HTMLButtonElement>('.merge-candidates button')]
    expect(candidateButtons).toHaveLength(3)
    candidateButtons[2]!.click()
    await nextTick()
    const final = host.querySelector<HTMLTextAreaElement>('.merge-final textarea')!
    expect(final.value).toBe('# Shared\n\nRemote\n')

    host.querySelector<HTMLButtonElement>('.three-way-resolver footer button')!.click()
    expect(resolved).toHaveBeenCalledWith('# Shared\n\nRemote\n')
  })

  it('preserves a manually edited final merge until the user explicitly regenerates it', async () => {
    const conflict: ConflictRecord = {
      path: 'notes/shared.md',
      kind: 'text',
      ancestor: '# Shared\n\nBase\n',
      local: '# Shared\n\nLocal\n',
      remote: '# Shared\n\nRemote\n',
      ancestorExists: true,
      localExists: true,
      remoteExists: true,
      recoveryId: 'manual-merge-recovery',
      choice: null,
    }
    const host = document.createElement('div')
    document.body.append(host)
    const app = createApp(ThreeWayConflictResolver, { conflict })
    app.use(i18n)
    app.mount(host)
    mounted.push(app)
    await nextTick()

    const final = host.querySelector<HTMLTextAreaElement>('.merge-final textarea')!
    final.value = 'Hand-edited final\n'
    final.dispatchEvent(new Event('input'))
    await nextTick()
    host.querySelector<HTMLButtonElement>('.merge-candidates button')!.click()
    await nextTick()

    expect(final.value).toBe('Hand-edited final\n')
    host.querySelector<HTMLButtonElement>('.merge-manual-state button')!.click()
    await nextTick()
    expect(final.value).not.toBe('Hand-edited final\n')
  })

  it('renders footnote references and definitions while keeping source untouched', async () => {
    Object.defineProperty(globalThis, 'ResizeObserver', {
      configurable: true,
      value: class {
        observe() {}
        unobserve() {}
        disconnect() {}
      },
    })
    const update = vi.fn()
    const source = 'Statement[^source].\n\n[^source]: Original citation\n'
    const host = document.createElement('div')
    document.body.append(host)
    const app = createApp(MarkdownEditor, {
      modelValue: source,
      loadWorkspaceImage: async () => '',
      markdown: true,
      'onUpdate:modelValue': update,
    })
    app.use(i18n)
    app.mount(host)
    mounted.push(app)
    await nextTick()

    expect(host.querySelector('.cm-footnote-reference')?.textContent).toBe('source')
    expect(host.querySelector('.cm-footnote-definition')?.textContent).toContain(
      'Original citation',
    )
    expect(update).not.toHaveBeenCalled()
  })

  it('opens a nested file from an expandable plain-workspace directory tree', async () => {
    const opened = vi.fn()
    const workspace: WorkspaceDescriptor = {
      id: 'plain-folder',
      name: '普通文件夹',
      root: 'E:\\Notes',
      git: null,
    }
    const entries: WorkspaceEntry[] = [
      {
        path: 'notes',
        name: 'notes',
        entryType: 'directory',
        fileKind: null,
        size: 0,
        modifiedMs: 1,
        readOnly: true,
        gitStatus: null,
      },
      {
        path: 'notes/today.txt',
        name: 'today.txt',
        entryType: 'file',
        fileKind: 'text',
        size: 12,
        modifiedMs: 2,
        readOnly: false,
        gitStatus: null,
      },
    ]
    const host = document.createElement('div')
    document.body.append(host)
    const app = createApp(WorkspaceSidebar, {
      workspace,
      entries,
      searchQuery: '',
      onOpenFile: opened,
    })
    app.use(i18n)
    app.mount(host)
    mounted.push(app)
    await nextTick()

    expect(host.querySelector('.sidebar-header span')?.textContent).toContain(
      i18n.global.t('app.localWorkspace'),
    )
    expect(host.querySelector('button[title="notes/today.txt"]')).toBeNull()

    host.querySelector<HTMLButtonElement>('button[title="notes"]')!.click()
    await nextTick()
    const file = host.querySelector<HTMLButtonElement>(
      'button[title="notes/today.txt"]',
    )
    expect(file).not.toBeNull()
    file!.click()
    expect(opened).toHaveBeenCalledWith('notes/today.txt')
    expect(host.querySelector('.file-state')).toBeNull()
  })

  it('shows a content search match even while its parent directory is collapsed', async () => {
    const opened = vi.fn()
    const workspace: WorkspaceDescriptor = {
      id: 'search-folder',
      name: 'Search notes',
      root: 'E:\\Search',
      git: null,
    }
    const host = document.createElement('div')
    document.body.append(host)
    const app = createApp(WorkspaceSidebar, {
      workspace,
      entries: [
        {
          path: 'hidden',
          name: 'hidden',
          entryType: 'directory',
          fileKind: null,
          size: 0,
          modifiedMs: 1,
          readOnly: true,
          gitStatus: null,
        },
        {
          path: 'hidden/match.md',
          name: 'match.md',
          entryType: 'file',
          fileKind: 'markdown',
          size: 12,
          modifiedMs: 2,
          readOnly: false,
          gitStatus: null,
        },
      ],
      searchQuery: 'needle',
      searchResults: [{
        worktree: 'Search notes',
        root: 'E:\\Search',
        path: 'hidden/match.md',
        line: 7,
        column: 2,
        snippet: 'A needle in the document',
        matchType: 'content',
        fileKind: 'markdown',
        modifiedMs: 2,
      }],
      onOpenSearchResult: opened,
    })
    app.use(i18n)
    app.mount(host)
    mounted.push(app)
    await nextTick()

    expect(host.querySelector('button[title="hidden/match.md"]')).not.toBeNull()
    expect(host.querySelector('.sidebar-search-row')?.textContent).toContain('needle')
    host.querySelector<HTMLButtonElement>('.sidebar-search-row')!.click()
    expect(opened).toHaveBeenCalledOnce()
  })

  it('renders only the visible window for a very large flat workspace', async () => {
    const workspace: WorkspaceDescriptor = {
      id: 'large-folder',
      name: 'Large',
      root: 'E:\\Large',
      git: null,
    }
    const entries: WorkspaceEntry[] = Array.from({ length: 2_000 }, (_, index) => ({
      path: `note-${index.toString().padStart(4, '0')}.md`,
      name: `note-${index.toString().padStart(4, '0')}.md`,
      entryType: 'file',
      fileKind: 'markdown',
      size: 12,
      modifiedMs: index,
      readOnly: false,
      gitStatus: null,
    }))
    const host = document.createElement('div')
    document.body.append(host)
    const app = createApp(WorkspaceSidebar, { workspace, entries, searchQuery: '' })
    app.use(i18n)
    app.mount(host)
    mounted.push(app)
    await nextTick()

    expect(host.querySelectorAll('.file-list .tree-row').length).toBeLessThanOrEqual(20)
  })

  it('keeps plain text literal and disables Markdown decoration', async () => {
    const update = vi.fn()
    const source = '# This is plain text\n[^not-a-footnote]\n'
    const host = document.createElement('div')
    document.body.append(host)
    const app = createApp(MarkdownEditor, {
      modelValue: source,
      loadWorkspaceImage: async () => '',
      markdown: false,
      'onUpdate:modelValue': update,
    })
    app.use(i18n)
    app.mount(host)
    mounted.push(app)
    await nextTick()

    expect(
      [...host.querySelectorAll('.cm-line')].map((line) => line.textContent),
    ).toEqual(['# This is plain text', '[^not-a-footnote]', ''])
    expect(host.querySelector('.cm-heading')).toBeNull()
    expect(host.querySelector('.cm-footnote-reference')).toBeNull()
    expect(update).not.toHaveBeenCalled()
  })

  it('rejects editor growth after the open-document memory budget is reached', async () => {
    const update = vi.fn()
    const limit = vi.fn()
    const host = document.createElement('div')
    document.body.append(host)
    const app = createApp(MarkdownEditor, {
      modelValue: 'full',
      documentCharacterLimit: 4,
      loadWorkspaceImage: async () => '',
      markdown: true,
      'onUpdate:modelValue': update,
      onLimit: limit,
    })
    app.use(i18n)
    const editor = app.mount(host) as unknown as { insertText: (text: string) => void }
    mounted.push(app)

    editor.insertText('!')
    await Promise.resolve()
    await nextTick()

    expect(update).not.toHaveBeenCalled()
    expect(limit).toHaveBeenCalledOnce()
    expect(host.querySelector('.cm-content')?.textContent).toBe('full')
  })

  it('creates a duplicate from the real file-tree context action', async () => {
    const requested = vi.fn()
    const workspace: WorkspaceDescriptor = {
      id: 'plain-folder',
      name: 'Notes',
      root: 'E:\\Notes',
      git: null,
    }
    const host = document.createElement('div')
    document.body.append(host)
    const app = createApp(WorkspaceSidebar, {
      workspace,
      entries: [
        {
          path: 'notes/note.md',
          name: 'note.md',
          entryType: 'file',
          fileKind: 'markdown',
          size: 12,
          modifiedMs: 2,
          readOnly: false,
          gitStatus: null,
        },
      ],
      searchQuery: '',
      onRequestEntryAction: requested,
    })
    app.use(i18n)
    app.mount(host)
    mounted.push(app)
    await nextTick()

    host
      .querySelector<HTMLButtonElement>('button[title="notes/note.md"]')!
      .dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, clientX: 12, clientY: 18 }))
    await nextTick()
    ;[...host.querySelectorAll<HTMLButtonElement>('.entry-context-menu button')]
      .find((button) => button.textContent?.includes(i18n.global.t('app.createDuplicate')))!
      .click()

    expect(requested).toHaveBeenCalledWith({
      action: 'duplicate',
      directory: 'notes',
      sourcePath: 'notes/note.md',
      entryName: 'note.md',
      entryType: 'file',
      suggestedName: `note ${i18n.global.t('app.copySuffix')}.md`,
    })
  })

  it('keeps a failed Git commit draft available for correction and retry', async () => {
    const runAction = vi.fn().mockResolvedValue(false)
    const host = document.createElement('div')
    document.body.append(host)
    const app = createApp(GitPanel, {
      status: {
        branch: 'main',
        upstream: 'origin/main',
        ahead: 0,
        behind: 0,
        stagedCount: 1,
        changedCount: 1,
        untrackedCount: 0,
        conflictedCount: 0,
        files: [{
          path: 'note.md',
          indexStatus: 'M',
          worktreeStatus: 'clean',
          staged: true,
          conflicted: false,
          untracked: false,
        }],
      },
      branches: [],
      worktrees: [],
      runAction,
      setStaged: vi.fn().mockResolvedValue(true),
      createBranch: vi.fn().mockResolvedValue(true),
      checkoutBranch: vi.fn().mockResolvedValue(true),
      deleteBranch: vi.fn().mockResolvedValue(true),
    })
    app.use(i18n)
    app.mount(host)
    mounted.push(app)
    const draft = host.querySelector<HTMLTextAreaElement>('.commit-box textarea')!
    draft.value = 'Keep this draft'
    draft.dispatchEvent(new Event('input'))
    await nextTick()
    host.querySelector<HTMLButtonElement>('.commit-box button')!.click()
    await Promise.resolve()
    await nextTick()

    expect(runAction).toHaveBeenCalledWith('commit', 'Keep this draft')
    expect(draft.value).toBe('Keep this draft')
  })

  it('applies mobile Markdown toolbar actions through the real editor buffer', async () => {
    const update = vi.fn()
    const host = document.createElement('div')
    document.body.append(host)
    const app = createApp(MarkdownEditor, {
      modelValue: '# Start\n',
      loadWorkspaceImage: async () => '',
      markdown: true,
      'onUpdate:modelValue': update,
    })
    app.use(i18n)
    app.mount(host)
    mounted.push(app)
    await nextTick()

    host
      .querySelector<HTMLButtonElement>(`button[title="${i18n.global.t('app.bold')}"]`)!
      .click()

    expect(update).toHaveBeenLastCalledWith('**text**# Start\n')
  })

  it('creates a personal snippet in the visible manager and inserts it into CodeMirror', async () => {
    const update = vi.fn()
    const host = document.createElement('div')
    document.body.append(host)
    const app = createApp(MarkdownEditor, {
      modelValue: '# Start\n',
      loadWorkspaceImage: async () => '',
      markdown: true,
      'onUpdate:modelValue': update,
    })
    app.use(i18n)
    app.mount(host)
    mounted.push(app)
    await nextTick()

    host
      .querySelector<HTMLButtonElement>(
        `button[title="${i18n.global.t('app.manageSnippets')}"]`,
      )!
      .click()
    await nextTick()
    const inputs = host.querySelectorAll<HTMLInputElement>('.snippet-form input')
    inputs[0]!.value = 'Callout'
    inputs[0]!.dispatchEvent(new Event('input'))
    inputs[1]!.value = 'callout'
    inputs[1]!.dispatchEvent(new Event('input'))
    const body = host.querySelector<HTMLTextAreaElement>('.snippet-form textarea')!
    body.value = '> {{selection}}Note{{cursor}}'
    body.dispatchEvent(new Event('input'))
    await nextTick()
    ;[...host.querySelectorAll<HTMLButtonElement>('.snippet-dialog footer button')]
      .find((button) => button.textContent?.includes(i18n.global.t('app.saveSnippet')))!
      .click()
    await nextTick()

    const snippetSelect = host.querySelector<HTMLSelectElement>('.markdown-toolbar select')!
    snippetSelect.value = markdownSnippets.value[0]!.id
    snippetSelect.dispatchEvent(new Event('change'))

    expect(markdownSnippets.value[0]).toMatchObject({
      name: 'Callout',
      shortcut: 'callout',
    })
    expect(update).toHaveBeenLastCalledWith('> Note# Start\n')
  })

  it('asks before an import can replace an unsaved snippet draft', async () => {
    markdownSnippets.value = [{
      id: 'saved-snippet',
      name: 'Saved',
      shortcut: 'saved',
      body: 'Saved body',
    }]
    const inputClick = vi
      .spyOn(HTMLInputElement.prototype, 'click')
      .mockImplementation(() => undefined)
    const host = document.createElement('div')
    document.body.append(host)
    const open = ref(false)
    const wrapper = defineComponent(() => () => h(SnippetManager, { open: open.value }))
    const app = createApp(wrapper)
    app.use(i18n)
    app.mount(host)
    mounted.push(app)
    open.value = true
    await nextTick()

    const name = host.querySelector<HTMLInputElement>('.snippet-form input')!
    name.value = 'Unsaved change'
    name.dispatchEvent(new Event('input'))
    await nextTick()
    ;[...host.querySelectorAll<HTMLButtonElement>('.snippet-dialog footer button')]
      .find((button) => button.textContent?.includes(i18n.global.t('app.replaceSnippets')))!
      .click()
    await nextTick()

    expect(inputClick).not.toHaveBeenCalled()
    expect(host.querySelector('[role="alertdialog"]')?.textContent).toContain(
      i18n.global.t('app.unsavedSnippetConfirm'),
    )
    host.querySelector<HTMLButtonElement>('.snippet-draft-warning .danger')!.click()
    expect(inputClick).toHaveBeenCalledOnce()
  })
})
