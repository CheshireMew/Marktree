import { afterEach, describe, expect, it, vi } from 'vitest'

import {
  duplicateWorkspaceEntry,
  loadDocuments,
  openDocument,
} from '../src/composables/workspace/documents'
import {
  activeWorktreePath,
  activeWorkspaceId,
  ensureSession,
  sessions,
  workspaces,
  workspaceForRoot,
} from '../src/composables/workspace/state'
import { nativeApi } from '../src/lib/api'
import { removeWorkspaceUiState, saveWorkspaceSession } from '../src/lib/workspaceUiState'
import { activateWorkspace } from '../src/composables/workspace/workspaces'
import type { GitStatusSnapshot, WorkspaceDescriptor } from '../src/types'

const cleanStatus: GitStatusSnapshot = {
  branch: 'main',
  upstream: null,
  ahead: 0,
  behind: 0,
  stagedCount: 0,
  changedCount: 0,
  untrackedCount: 0,
  conflictedCount: 0,
  files: [],
}

afterEach(() => {
  vi.restoreAllMocks()
  workspaces.value = []
  activeWorkspaceId.value = undefined
  activeWorktreePath.value = undefined
  sessions.clear()
  Reflect.deleteProperty(window, '__TAURI_INTERNALS__')
})

describe('workspace root capability ownership', () => {
  it('does not try to open a duplicated directory even when it contains one file', async () => {
    const plain: WorkspaceDescriptor = {
      id: 'plain',
      name: 'Plain',
      root: 'E:\\Plain',
      git: null,
    }
    workspaces.value = [plain]
    activeWorkspaceId.value = plain.id
    const session = ensureSession(plain.root)
    session.entries = [
      {
        path: 'notes',
        name: 'notes',
        entryType: 'directory',
        fileKind: null,
        size: 0,
        modifiedMs: 1,
        readOnly: false,
        gitStatus: null,
      },
      {
        path: 'notes/only.md',
        name: 'only.md',
        entryType: 'file',
        fileKind: 'markdown',
        size: 4,
        modifiedMs: 1,
        readOnly: false,
        gitStatus: null,
      },
    ]
    Object.defineProperty(window, '__TAURI_INTERNALS__', {
      configurable: true,
      value: {},
    })
    vi.spyOn(nativeApi, 'duplicateWorkspaceEntry').mockResolvedValue({
      sourcePath: 'notes',
      newPath: 'notes-copy',
      copiedFiles: [{ oldPath: 'notes/only.md', newPath: 'notes-copy/only.md' }],
    })
    vi.spyOn(nativeApi, 'workspaceView').mockResolvedValue({
      entries: [],
      status: null,
      branches: [],
      pendingOperation: null,
      conflicts: [],
    })
    const textOpen = vi.spyOn(nativeApi, 'openDocument')

    const result = await duplicateWorkspaceEntry('notes', 'notes-copy')

    expect(result).toBe(true)
    expect(textOpen).not.toHaveBeenCalled()
  })

  it('never borrows Git capability from the currently selected workspace', async () => {
    const plain: WorkspaceDescriptor = {
      id: 'plain',
      name: 'Plain',
      root: 'E:\\Plain',
      git: null,
    }
    const git: WorkspaceDescriptor = {
      id: 'git',
      name: 'Git',
      root: 'E:\\Git',
      git: {
        commonDir: 'E:\\Git\\.git',
        remoteUrl: null,
        status: cleanStatus,
        worktrees: [
          {
            name: 'main',
            path: 'E:\\Git',
            branch: 'main',
            isMain: true,
            isLocked: false,
            isDetached: false,
            status: cleanStatus,
          },
        ],
      },
    }
    workspaces.value = [plain, git]
    activeWorkspaceId.value = git.id
    Object.defineProperty(window, '__TAURI_INTERNALS__', {
      configurable: true,
      value: {},
    })
    const workspaceView = vi
      .spyOn(nativeApi, 'workspaceView')
      .mockResolvedValue({
        entries: [],
        status: null,
        branches: [],
        pendingOperation: null,
        conflicts: [],
      })
    const pending = vi.spyOn(nativeApi, 'pendingGitOperation').mockResolvedValue(null)

    await loadDocuments(plain.root)

    expect(workspaceForRoot(plain.root)?.id).toBe(plain.id)
    expect(workspaceView).toHaveBeenCalledWith({ root: plain.root })
    expect(pending).not.toHaveBeenCalled()
  })

  it('opens unsupported workspace files with the system instead of treating them as text', async () => {
    const plain: WorkspaceDescriptor = {
      id: 'plain',
      name: 'Plain',
      root: 'E:\\Plain',
      git: null,
    }
    workspaces.value = [plain]
    activeWorkspaceId.value = plain.id
    ensureSession(plain.root).entries = [
      {
        path: 'archive.bin',
        name: 'archive.bin',
        entryType: 'file',
        fileKind: 'other',
        size: 4,
        modifiedMs: 1,
        readOnly: true,
        gitStatus: null,
      },
    ]
    Object.defineProperty(window, '__TAURI_INTERNALS__', {
      configurable: true,
      value: {},
    })
    const systemOpen = vi
      .spyOn(nativeApi, 'openWorkspaceFileWithSystem')
      .mockResolvedValue(undefined)
    const textOpen = vi.spyOn(nativeApi, 'openDocument')

    await openDocument('archive.bin')

    expect(systemOpen).toHaveBeenCalledWith({
      root: plain.root,
      path: 'archive.bin',
    })
    expect(textOpen).not.toHaveBeenCalled()
  })

  it('restores persisted tab paths through the real document opener and selects the saved tab', async () => {
    const restored: WorkspaceDescriptor = {
      id: 'restore',
      name: 'Restore',
      root: 'E:\\Restore',
      git: null,
    }
    workspaces.value = [restored]
    saveWorkspaceSession(restored.root, ['a.md', 'b.md'], 'a.md')
    Object.defineProperty(window, '__TAURI_INTERNALS__', {
      configurable: true,
      value: {},
    })
    vi.spyOn(nativeApi, 'workspaceView').mockResolvedValue({
      entries: [{
        path: 'a.md',
        name: 'a.md',
        entryType: 'file',
        fileKind: 'markdown',
        size: 1,
        modifiedMs: 1,
        readOnly: false,
        gitStatus: null,
      }, {
        path: 'b.md',
        name: 'b.md',
        entryType: 'file',
        fileKind: 'markdown',
        size: 1,
        modifiedMs: 1,
        readOnly: false,
        gitStatus: null,
      }],
      status: null,
      branches: [],
      pendingOperation: null,
      conflicts: [],
    })
    const open = vi.spyOn(nativeApi, 'openDocument').mockImplementation(async ({ path }) => ({
      path,
      content: `# ${path}`,
      modifiedMs: 1,
      sha256: `hash-${path}`,
      readOnly: false,
      encoding: 'utf8',
      lineEnding: 'lf',
    }))

    await activateWorkspace(restored.id)

    expect(open.mock.calls.map(([request]) => request.path)).toEqual(['a.md', 'b.md'])
    expect(ensureSession(restored.root).tabs.map((item) => item.path)).toEqual(['a.md', 'b.md'])
    expect(ensureSession(restored.root).activeTabKey).toBe(`${restored.root}\na.md`)
    removeWorkspaceUiState([restored.root])
  })

  it('coalesces concurrent opens of the same document into one tab', async () => {
    const plain: WorkspaceDescriptor = {
      id: 'plain',
      name: 'Plain',
      root: 'E:\\Plain',
      git: null,
    }
    workspaces.value = [plain]
    activeWorkspaceId.value = plain.id
    ensureSession(plain.root).entries = [{
      path: 'note.md',
      name: 'note.md',
      entryType: 'file',
      fileKind: 'markdown',
      size: 1,
      modifiedMs: 1,
      readOnly: false,
      gitStatus: null,
    }]
    Object.defineProperty(window, '__TAURI_INTERNALS__', {
      configurable: true,
      value: {},
    })
    let release: (() => void) | undefined
    const gate = new Promise<void>((resolve) => { release = resolve })
    const nativeOpen = vi.spyOn(nativeApi, 'openDocument').mockImplementation(async () => {
      await gate
      return {
        path: 'note.md',
        content: '# Note',
        modifiedMs: 1,
        sha256: 'hash',
        readOnly: false,
        encoding: 'utf8',
        lineEnding: 'lf',
      }
    })

    const first = openDocument('note.md')
    const second = openDocument('note.md')
    release?.()
    await Promise.all([first, second])

    expect(nativeOpen).toHaveBeenCalledTimes(1)
    expect(ensureSession(plain.root).tabs).toHaveLength(1)
  })
})
