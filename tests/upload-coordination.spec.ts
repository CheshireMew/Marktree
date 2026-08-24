import { afterEach, describe, expect, it, vi } from 'vitest'

import { writeImage } from '../src/composables/workspace/documents'
import {
  activeWorkspaceId,
  ensureSession,
  sessions,
  tabKey,
  workspaces,
} from '../src/composables/workspace/state'
import {
  trackTabUpload,
  waitForUploadsUnderPath,
} from '../src/composables/workspace/uploads'
import { nativeApi } from '../src/lib/api'
import { disposeSession } from '../src/composables/workspace/persistence'
import type { EditorTab, WorkspaceDescriptor } from '../src/types'

function editorTab(root: string): EditorTab {
  return {
    root,
    path: 'note.md',
    title: 'note.md',
    content: 'before after',
    diskContent: 'before after',
    modifiedMs: 1,
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

afterEach(() => {
  vi.restoreAllMocks()
  vi.useRealTimers()
  workspaces.value = []
  activeWorkspaceId.value = undefined
  for (const root of sessions.keys()) disposeSession(root)
  Reflect.deleteProperty(window, '__TAURI_INTERNALS__')
})

describe('asset upload ownership', () => {
  it('keeps path operations pending until every upload owned by the tab settles', async () => {
    const root = 'E:\\Notes'
    const tab = editorTab(root)
    ensureSession(root).tabs.push(tab)
    let release: (() => void) | undefined
    const upload = new Promise<void>((resolve) => { release = resolve })
    trackTabUpload(tab, upload)
    let pathReady = false

    const waiting = waitForUploadsUnderPath(root, 'note.md').then(() => {
      pathReady = true
    })
    await Promise.resolve()
    expect(pathReady).toBe(false)

    release?.()
    await waiting
    expect(pathReady).toBe(true)
  })

  it('replaces a stable placeholder and preserves edits made while bytes upload', async () => {
    const root = 'E:\\Notes'
    const descriptor: WorkspaceDescriptor = {
      id: 'notes',
      name: 'Notes',
      root,
      git: null,
    }
    workspaces.value = [descriptor]
    activeWorkspaceId.value = descriptor.id
    const tab = editorTab(root)
    const session = ensureSession(root)
    session.tabs.push(tab)
    session.activeTabKey = tabKey(root, tab.path)
    Object.defineProperty(window, '__TAURI_INTERNALS__', {
      configurable: true,
      value: {},
    })
    vi.spyOn(nativeApi, 'beginAssetUpload').mockResolvedValue({
      id: 'upload',
      maxChunkBytes: 2,
    })
    const append = vi.spyOn(nativeApi, 'appendAssetUpload').mockResolvedValue(undefined)
    let finish: ((value: { path: string; markdownPath: string; sha256: string }) => void) | undefined
    vi.spyOn(nativeApi, 'finishAssetUpload').mockImplementation(() =>
      new Promise((resolve) => { finish = resolve }),
    )

    const writing = writeImage(new File([new Uint8Array([1, 2, 3])], 'photo.png'), 7)
    await vi.waitFor(() => expect(finish).toBeTypeOf('function'))
    expect(tab.content).toContain('marktree-upload:')
    tab.content = `live edit ${tab.content}`
    tab.revision += 1
    finish?.({
      path: 'assets/photo.png',
      markdownPath: 'assets/photo.png',
      sha256: 'asset-hash',
    })
    await writing

    expect(append.mock.calls.map(([call]) => call.request.offset)).toEqual([0, 2])
    expect(tab.content).toBe('live edit before ![photo](assets/photo.png)after')
    expect(tab.content).not.toContain('marktree-upload:')
  })
})
