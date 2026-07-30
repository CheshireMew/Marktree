import { createApp, nextTick } from 'vue'
import { afterEach, describe, expect, it, vi } from 'vitest'

import ConflictDialog from '../src/components/ConflictDialog.vue'
import MarkdownEditor from '../src/components/MarkdownEditor.vue'
import ThreeWayConflictResolver from '../src/components/ThreeWayConflictResolver.vue'
import WorkspaceSidebar from '../src/components/WorkspaceSidebar.vue'
import { i18n } from '../src/i18n'
import type {
  ConflictRecord,
  WorkspaceDescriptor,
  WorkspaceEntry,
} from '../src/types'

const mounted: Array<ReturnType<typeof createApp>> = []

afterEach(() => {
  for (const app of mounted.splice(0)) app.unmount()
  document.body.replaceChildren()
})

describe('user-visible component interactions', () => {
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
})
