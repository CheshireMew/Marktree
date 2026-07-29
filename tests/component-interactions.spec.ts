import { createApp, nextTick } from 'vue'
import { afterEach, describe, expect, it, vi } from 'vitest'

import ConflictDialog from '../src/components/ConflictDialog.vue'
import MarkdownEditor from '../src/components/MarkdownEditor.vue'
import ThreeWayConflictResolver from '../src/components/ThreeWayConflictResolver.vue'
import { i18n } from '../src/i18n'
import type { ConflictRecord } from '../src/types'

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
      loadRepositoryImage: async () => '',
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
})
