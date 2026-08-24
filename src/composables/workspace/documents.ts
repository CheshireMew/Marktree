import { i18n } from '@/i18n'
import { convertFileSrc } from '@tauri-apps/api/core'
import { isTauri, nativeApi } from '@/lib/api'
import {
  canOpenDocument,
  editorTabIsDirty,
  retainedDiskContent,
} from '@/lib/documentMemory'
import {
  migrateWorkspacePaths,
  removeFavoritePaths,
} from '@/lib/workspaceUiState'
import {
  activeRoot,
  activeTab,
  beginLoading,
  endLoading,
  ensureSession,
  fileName,
  filePreview,
  externalComparisons,
  persistSessionPaths,
  recentFiles,
  recentFileLimit,
  retainedEditorTabs,
  sessions,
  setError,
  setNotice,
  tabKey,
  trashEntries,
  updateWorktreeStatus,
  workspaceForRoot,
} from './state'
import { flushAll, reconcileOpenTabs, scheduleSave } from './persistence'
import { trackTabUpload, waitForUploadsUnderPath } from './uploads'
import { demoContents } from './demoData'
import type { WorkspaceEntry, WorkspaceViewSnapshot, WorktreeSearchResult } from '@/types'

const openDocumentTasks = new Map<string, Promise<void>>()
let uploadPlaceholderSequence = 0

export async function loadDocuments(
  requestedRoot = activeRoot.value,
  preloaded?: WorkspaceViewSnapshot,
  changedPaths?: ReadonlySet<string>,
) {
  const root = requestedRoot
  if (!root || !isTauri()) return
  const session = ensureSession(root)
  const generation = ++session.loadGeneration
  beginLoading()
  try {
    const view = preloaded ?? await nativeApi.workspaceView({ root })
    const nextEntries = view.entries
    const pendingOperation = view.pendingOperation
    const pendingConflicts = view.conflicts
    if (session.loadGeneration !== generation) return
    if (!pendingOperation && !pendingConflicts.length) {
      await reconcileOpenTabs(session, nextEntries, generation, changedPaths)
      if (session.loadGeneration !== generation) return
    }
    if (view.status) updateWorktreeStatus(root, view.status)
    session.branches = view.branches
    session.conflicts = pendingConflicts
    session.pendingOperation = pendingOperation ?? undefined
    session.entries = nextEntries
    if (pendingOperation && !pendingConflicts.length) {
      setNotice(i18n.global.t('app.pendingGitOperation'))
    }
  } catch (reason) {
    setError(reason)
  } finally {
    endLoading()
  }
}

export async function loadWorkspaceImage(root: string, path: string) {
  const preview = await nativeApi.readWorkspacePreview({ root, path })
  return convertFileSrc(preview.resourcePath)
}

export async function loadWorkspacePreview(root: string, path: string) {
  const preview = await nativeApi.readWorkspacePreview({ root, path })
  return {
    root,
    path,
    kind: preview.kind,
    mediaType: preview.mediaType,
    url: convertFileSrc(preview.resourcePath),
  }
}

interface OpenDocumentOptions {
  activate?: boolean
  remember?: boolean
  root?: string
}

export async function openDocument(path: string, options: OpenDocumentOptions = {}) {
  const activate = options.activate ?? true
  const remember = options.remember ?? true
  const root = options.root ?? activeRoot.value
  if (!root) return
  const descriptor = ensureSession(root).entries.find(
    (entry) => entry.path === path,
  )
  if (
    descriptor?.fileKind &&
    ['image', 'pdf', 'audio', 'video'].includes(descriptor.fileKind)
  ) {
    try {
      filePreview.value = await loadWorkspacePreview(root, path)
    } catch (reason) {
      setError(reason)
    }
    return
  }
  if (descriptor?.fileKind === 'other') {
    try {
      await nativeApi.openWorkspaceFileWithSystem({ root, path })
    } catch (reason) {
      setError(reason)
    }
    return
  }
  const key = tabKey(root, path)
  const session = ensureSession(root)
  const existing = session.tabs.find((tab) => tabKey(tab.root, tab.path) === key)
  if (existing) {
    if (activate) {
      session.activeTabKey = key
      if (remember) rememberRecent(key)
      persistSessionPaths(session)
    }
    return
  }
  if (!isTauri()) {
    const demo = demoContents[path]
    if (demo === undefined) return
    if (!canOpenDocument(retainedEditorTabs(), demo)) {
      setError(i18n.global.t('app.openDocumentLimit'))
      return
    }
    session.tabs.push({
      root,
      path,
      title: fileName(path),
      content: demo,
      diskContent: retainedDiskContent(demo),
      modifiedMs: Date.now(),
      sha256: 'demo',
      readOnly: false,
      encoding: 'utf8',
      lineEnding: 'lf',
      revision: 0,
      savedRevision: 0,
      dirty: false,
      saving: false,
    })
    if (activate) {
      session.activeTabKey = key
      if (remember) rememberRecent(key)
      persistSessionPaths(session)
    }
    return
  }
  try {
    const pending = openDocumentTasks.get(key)
    if (pending) {
      await pending
      return
    }
    const task = (async () => {
      const content = remember
        ? await nativeApi.openDocument({ root, path })
        : await nativeApi.readDocument({ root, path })
      if (!sessions.has(root)) return
      const currentSession = ensureSession(root)
      const opened = currentSession.tabs.find((tab) => tabKey(tab.root, tab.path) === key)
      if (!opened) {
        if (!canOpenDocument(retainedEditorTabs(), content.content)) {
          throw new Error(i18n.global.t('app.openDocumentLimit'))
        }
        currentSession.tabs.push({
          ...content,
          root,
          title: fileName(path),
          diskContent: retainedDiskContent(content.content),
          revision: 0,
          savedRevision: 0,
          dirty: false,
          saving: false,
        })
      }
      if (activate) {
        currentSession.activeTabKey = key
        if (remember) rememberRecent(key)
        persistSessionPaths(currentSession)
      }
    })().finally(() => openDocumentTasks.delete(key))
    openDocumentTasks.set(key, task)
    await task
  } catch (reason) {
    setError(reason)
  }
}

export function rememberRecent(key: string) {
  recentFiles.value = [key, ...recentFiles.value.filter((item) => item !== key)].slice(
    0,
    recentFileLimit.value,
  )
}

export async function createDocument(path: string): Promise<boolean> {
  const root = activeRoot.value
  if (!root || !isTauri()) return false
  try {
    const normalized = /\.[a-z0-9]+$/i.test(path) ? path : `${path}.md`
    const content = await nativeApi.createDocument({ root, path: normalized })
    await loadDocuments(root)
    if (activeRoot.value === root) await openDocument(content.path)
    return true
  } catch (reason) {
    setError(reason)
    return false
  }
}

export async function writeImage(file: File, cursor: number) {
  const tab = activeTab.value
  if (!tab || !isTauri()) return
  const placeholderId = `${Date.now().toString(36)}-${++uploadPlaceholderSequence}`
  const placeholder = `<!-- marktree-upload:${placeholderId} -->`
  const insertAt = Math.max(0, Math.min(cursor, tab.content.length))
  tab.content = `${tab.content.slice(0, insertAt)}${placeholder}${tab.content.slice(insertAt)}`
  tab.revision += 1
  tab.dirty = editorTabIsDirty(tab)

  const task = (async () => {
    let uploadId: string | undefined
    try {
      const upload = await nativeApi.beginAssetUpload({
        request: {
          root: tab.root,
          documentPath: tab.path,
          fileName: file.name,
          assetsDir: null,
          totalBytes: file.size,
        },
      })
      uploadId = upload.id
      const uploadChunkBytes = upload.maxChunkBytes
      for (let offset = 0; offset < file.size; offset += uploadChunkBytes) {
        const chunk = new Uint8Array(
          await file.slice(offset, offset + uploadChunkBytes).arrayBuffer(),
        )
        let binary = ''
        for (let index = 0; index < chunk.length; index += 0x8000) {
          binary += String.fromCharCode(...chunk.subarray(index, index + 0x8000))
        }
        await nativeApi.appendAssetUpload({
          request: {
            uploadId,
            offset,
            base64Data: btoa(binary),
          },
        })
      }
      const result = await nativeApi.finishAssetUpload({ uploadId })
      uploadId = undefined
      const markdown = `![${file.name.replace(/\.[^.]+$/, '')}](${result.markdownPath})`
      if (tab.content.includes(placeholder)) {
        tab.content = tab.content.replace(placeholder, markdown)
        tab.revision += 1
        tab.dirty = editorTabIsDirty(tab)
      }
    } catch (reason) {
      if (uploadId) {
        await nativeApi.abortAssetUpload({ uploadId }).catch(() => undefined)
      }
      if (tab.content.includes(placeholder)) {
        tab.content = tab.content.replace(placeholder, '')
        tab.revision += 1
        tab.dirty = editorTabIsDirty(tab)
      }
      setError(reason)
      throw reason
    } finally {
      scheduleSave(tab)
    }
  })()
  trackTabUpload(tab, task)
  scheduleSave(tab)
  await task.catch(() => undefined)
}

export async function search() {
  const root = activeRoot.value
  const session = root ? ensureSession(root) : undefined
  if (!root || !session || !isTauri() || !session.searchQuery.trim()) {
    if (session) {
      session.searchMatches = []
      session.crossWorktreeMatches = []
      session.searching = false
    }
    return
  }
  const generation = ++session.searchGeneration
  const query = session.searchQuery
  session.searching = true
  try {
    const owner = workspaceForRoot(root)
    let matches: WorktreeSearchResult[]
    let truncated: boolean
    if (owner?.git) {
      const response = await nativeApi.searchWorktrees({
          request: {
            root: owner.root,
            query,
            limit: 120,
            pathPrefix: null,
            fileKinds: [],
            modifiedAfterMs: null,
          },
        })
      matches = response.results
      truncated = response.statistics.truncated
    } else {
      const response = await nativeApi.searchDocuments({
          request: {
            root,
            query,
            limit: 120,
            pathPrefix: null,
            fileKinds: [],
            modifiedAfterMs: null,
          },
        })
      matches = response.results.map((result) => ({
        worktree: owner?.name ?? '',
        root,
        ...result,
      }))
      truncated = response.statistics.truncated
    }
    if (session.searchGeneration !== generation || session.searchQuery !== query) return
    session.crossWorktreeMatches = matches
    session.searchMatches = matches.filter((match) => match.root === root)
    if (truncated) {
      setNotice(i18n.global.t('app.searchLimited'))
    }
  } catch (reason) {
    setError(reason)
  } finally {
    if (session.searchGeneration === generation) session.searching = false
  }
}

export async function handleWorkspaceChanged(root: string, paths?: string[]) {
  if (!paths?.length) {
    await loadDocuments(root)
    return
  }
  const session = ensureSession(root)
  if (session.pendingOperation || session.conflicts.length) {
    await loadDocuments(root, undefined, new Set(paths))
    return
  }
  const generation = ++session.loadGeneration
  try {
    const patch = await nativeApi.workspaceEntriesPatch({ root, paths })
    if (patch.fullReloadRequired) {
      await loadDocuments(root, undefined, new Set(paths))
      return
    }
    if (session.loadGeneration !== generation) return
    const removed = patch.removedPaths
    const entries = new Map(
      session.entries
        .filter((entry) => !removed.some((path) => pathContains(path, entry.path)))
        .map((entry) => [entry.path, entry]),
    )
    for (const entry of patch.entries) entries.set(entry.path, entry)
    const nextEntries = [...entries.values()].sort(compareWorkspaceEntries)
    await reconcileOpenTabs(session, nextEntries, generation, new Set(paths))
    if (session.loadGeneration !== generation) return
    if (patch.status) updateWorktreeStatus(root, patch.status)
    session.entries = nextEntries
  } catch (reason) {
    setError(reason)
  }
}

function compareWorkspaceEntries(left: WorkspaceEntry, right: WorkspaceEntry) {
  const leftRank = left.entryType === 'file' ? 1 : 0
  const rightRank = right.entryType === 'file' ? 1 : 0
  return leftRank - rightRank || left.path.localeCompare(right.path, undefined, { sensitivity: 'base' })
}

export async function createFolder(path: string): Promise<boolean> {
  const root = activeRoot.value
  if (!root || !isTauri()) return false
  beginLoading()
  try {
    await nativeApi.createWorkspaceFolder({ root, path })
    await loadDocuments(root)
    return true
  } catch (reason) {
    setError(reason)
    return false
  } finally {
    endLoading()
  }
}

export async function moveWorkspaceEntry(
  sourcePath: string,
  destinationPath: string,
): Promise<boolean> {
  const root = activeRoot.value
  if (!root || !isTauri()) return false
  beginLoading()
  try {
    await prepareEntryOperation(root, sourcePath)
    const result = await nativeApi.moveWorkspaceEntry({
      request: { root, sourcePath, destinationPath },
    })
    migrateOpenPaths(root, result.movedFiles)
    migrateWorkspacePaths(root, result.movedFiles)
    await loadDocuments(root)
    return true
  } catch (reason) {
    setError(reason)
    return false
  } finally {
    endLoading()
  }
}

export async function duplicateWorkspaceEntry(
  sourcePath: string,
  destinationPath: string,
): Promise<boolean> {
  const root = activeRoot.value
  if (!root || !isTauri()) return false
  beginLoading()
  try {
    const sourceEntry = ensureSession(root).entries.find(
      (entry) => entry.path === sourcePath,
    )
    await flushAll(root)
    const result = await nativeApi.duplicateWorkspaceEntry({
      request: { root, sourcePath, destinationPath },
    })
    await loadDocuments(root)
    if (sourceEntry?.entryType === 'file' && result.copiedFiles.length === 1) {
      await openDocument(result.copiedFiles[0].newPath)
    }
    return true
  } catch (reason) {
    setError(reason)
    return false
  } finally {
    endLoading()
  }
}

export async function trashWorkspaceEntry(path: string): Promise<boolean> {
  const root = activeRoot.value
  if (!root || !isTauri()) return false
  beginLoading()
  try {
    await prepareEntryOperation(root, path)
    await nativeApi.trashWorkspaceEntry({ root, path })
    trashEntries.value = await nativeApi.listWorkspaceTrash()
    const session = ensureSession(root)
    session.tabs = session.tabs.filter((tab) => !pathContains(path, tab.path))
    if (session.activeTabKey && pathContains(path, session.activeTabKey.split('\n')[1] ?? '')) {
      session.activeTabKey = undefined
    }
    recentFiles.value = recentFiles.value.filter((key) => {
      const [keyRoot, keyPath] = key.split('\n')
      return keyRoot !== root || !pathContains(path, keyPath ?? '')
    })
    removeFavoritePaths(root, path)
    persistSessionPaths(session)
    await loadDocuments(root)
    return true
  } catch (reason) {
    setError(reason)
    return false
  } finally {
    endLoading()
  }
}

export async function openWithSystem(path: string) {
  const root = activeRoot.value
  if (!root || !isTauri()) return
  try {
    await nativeApi.openWorkspaceFileWithSystem({ root, path })
  } catch (reason) {
    setError(reason)
  }
}

export async function loadWorkspaceTrash() {
  if (!isTauri()) return
  try {
    trashEntries.value = await nativeApi.listWorkspaceTrash()
  } catch (reason) {
    setError(reason)
  }
}

export async function restoreWorkspaceTrash(id: string) {
  if (!isTauri()) return
  try {
    const restored = await nativeApi.restoreWorkspaceTrash({ id })
    trashEntries.value = await nativeApi.listWorkspaceTrash()
    if (restored.workspaceRoot === activeRoot.value) {
      await loadDocuments(restored.workspaceRoot)
    }
  } catch (reason) {
    setError(reason)
  }
}

export async function emptyWorkspaceTrash(): Promise<boolean> {
  if (!isTauri()) return false
  try {
    await nativeApi.emptyWorkspaceTrash()
    trashEntries.value = []
    return true
  } catch (reason) {
    setError(reason)
    return false
  }
}

async function prepareEntryOperation(root: string, path: string) {
  if (
    externalComparisons.value.some(
      (comparison) => comparison.root === root && pathContains(path, comparison.path),
    )
  ) {
    throw new Error(i18n.global.t('app.resolveExternalChangeFirst'))
  }
  await waitForUploadsUnderPath(root, path)
  await flushAll(root)
}

function migrateOpenPaths(
  root: string,
  moves: Array<{ oldPath: string; newPath: string }>,
) {
  const session = ensureSession(root)
  const byOldPath = new Map(moves.map((move) => [move.oldPath, move.newPath]))
  for (const tab of session.tabs) {
    const nextPath = byOldPath.get(tab.path)
    if (!nextPath) continue
    const wasActive = session.activeTabKey === tabKey(root, tab.path)
    tab.path = nextPath
    tab.title = fileName(nextPath)
    if (wasActive) session.activeTabKey = tabKey(root, nextPath)
  }
  recentFiles.value = recentFiles.value.map((key) => {
    const [keyRoot, keyPath] = key.split('\n')
    const nextPath = keyRoot === root ? byOldPath.get(keyPath ?? '') : undefined
    return nextPath ? tabKey(root, nextPath) : key
  })
  if (filePreview.value?.root === root) {
    const nextPath = byOldPath.get(filePreview.value.path)
    if (nextPath) filePreview.value.path = nextPath
  }
  persistSessionPaths(session)
}

function pathContains(parent: string, candidate: string) {
  return candidate === parent || candidate.startsWith(`${parent}/`)
}
