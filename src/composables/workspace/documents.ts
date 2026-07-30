import { i18n } from '@/i18n'
import { isTauri, nativeApi } from '@/lib/api'
import {
  activeWorkspace,
  activeRoot,
  activeTab,
  beginLoading,
  endLoading,
  ensureSession,
  fileName,
  imagePreview,
  message,
  externalComparisons,
  recentFiles,
  setError,
  tabKey,
  trashEntries,
  updateWorktreeStatus,
} from './state'
import { flushAll, reconcileOpenTabs, scheduleSave } from './persistence'
import { demoContents } from './demoData'

export async function loadDocuments(requestedRoot = activeRoot.value) {
  const root = requestedRoot
  if (!root || !isTauri()) return
  const session = ensureSession(root)
  const generation = ++session.loadGeneration
  beginLoading()
  try {
    const nextEntries = await nativeApi.listWorkspaceEntries({ root })
    const gitEnabled = Boolean(activeWorkspace.value?.git)
    const [status, branchList, pendingOperation, pendingConflicts] = gitEnabled
      ? await Promise.all([
          nativeApi.workspaceGitStatus({ root }),
          nativeApi.listBranches({ root }),
          nativeApi.pendingGitOperation({ root }),
          nativeApi.pendingConflicts({ root }),
        ])
      : [undefined, [], null, []]
    if (session.loadGeneration !== generation) return
    if (!pendingOperation && !pendingConflicts.length) {
      await reconcileOpenTabs(session, nextEntries, generation)
      if (session.loadGeneration !== generation) return
    }
    if (status) updateWorktreeStatus(root, status)
    session.branches = branchList
    session.conflicts = pendingConflicts
    session.pendingOperation = pendingOperation ?? undefined
    session.entries = nextEntries
    if (pendingOperation && !pendingConflicts.length) {
      message.value = i18n.global.t('app.pendingGitOperation')
    }
  } catch (reason) {
    setError(reason)
  } finally {
    endLoading()
  }
}

export async function loadWorkspaceImage(root: string, path: string) {
  const preview = await nativeApi.readAsset({ root, path })
  return `data:${preview.mediaType};base64,${preview.base64Data}`
}

export async function openDocument(path: string) {
  const root = activeRoot.value
  if (!root) return
  const descriptor = ensureSession(root).entries.find(
    (entry) => entry.path === path,
  )
  if (descriptor?.fileKind === 'image') {
    try {
      imagePreview.value = {
        root,
        path,
        url: await loadWorkspaceImage(root, path),
      }
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
    session.activeTabKey = key
    rememberRecent(key)
    return
  }
  if (!isTauri()) {
    const demo = demoContents[path]
    if (demo === undefined) return
    session.tabs.push({
      root,
      path,
      title: fileName(path),
      content: demo,
      diskContent: demo,
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
    session.activeTabKey = key
    rememberRecent(key)
    return
  }
  try {
    const content = await nativeApi.openDocument({ root, path })
    session.tabs.push({
      ...content,
      root,
      title: fileName(path),
      diskContent: content.content,
      revision: 0,
      savedRevision: 0,
      dirty: false,
      saving: false,
    })
    session.activeTabKey = key
    rememberRecent(key)
  } catch (reason) {
    setError(reason)
  }
}

export function rememberRecent(key: string) {
  recentFiles.value = [key, ...recentFiles.value.filter((item) => item !== key)].slice(0, 40)
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
  try {
    const data = await file.arrayBuffer()
    const bytes = new Uint8Array(data)
    let binary = ''
    for (let index = 0; index < bytes.length; index += 0x8000) {
      binary += String.fromCharCode(...bytes.subarray(index, index + 0x8000))
    }
    const result = await nativeApi.writeAsset({
      root: tab.root,
      documentPath: tab.path,
      fileName: file.name,
      base64Data: btoa(binary),
      assetsDir: null,
    })
    const markdown = `![${file.name.replace(/\.[^.]+$/, '')}](${result.markdownPath})`
    tab.content = `${tab.content.slice(0, cursor)}${markdown}${tab.content.slice(cursor)}`
    tab.revision += 1
    tab.dirty = true
    scheduleSave(tab)
  } catch (reason) {
    setError(reason)
  }
}

export async function search() {
  const root = activeRoot.value
  const session = root ? ensureSession(root) : undefined
  if (!root || !session || !isTauri() || !session.searchQuery.trim()) {
    if (session) {
      session.searchMatches = []
      session.crossWorktreeMatches = []
    }
    return
  }
  const generation = ++session.searchGeneration
  const query = session.searchQuery
  try {
    const matches = activeWorkspace.value?.git
      ? await nativeApi.searchWorktrees({
          root: activeWorkspace.value.root,
          query,
          limit: 120,
        })
      : (await nativeApi.searchDocuments({ root, query, limit: 120 })).map((path) => ({
          worktree: activeWorkspace.value?.name ?? '',
          root,
          path,
        }))
    if (session.searchGeneration !== generation || session.searchQuery !== query) return
    session.crossWorktreeMatches = matches
    session.searchMatches = matches
      .filter((match) => match.root === root)
      .map((match) => match.path)
  } catch (reason) {
    setError(reason)
  }
}

export async function handleWorkspaceChanged(root: string) {
  await loadDocuments(root)
}

export async function createFolder(path: string): Promise<boolean> {
  const root = activeRoot.value
  if (!root || !isTauri()) return false
  try {
    await nativeApi.createWorkspaceFolder({ root, path })
    await loadDocuments(root)
    return true
  } catch (reason) {
    setError(reason)
    return false
  }
}

export async function moveWorkspaceEntry(
  sourcePath: string,
  destinationPath: string,
): Promise<boolean> {
  const root = activeRoot.value
  if (!root || !isTauri()) return false
  try {
    await prepareEntryOperation(root, sourcePath)
    const result = await nativeApi.moveWorkspaceEntry({
      request: { root, sourcePath, destinationPath },
    })
    migrateOpenPaths(root, result.movedFiles)
    await loadDocuments(root)
    return true
  } catch (reason) {
    setError(reason)
    return false
  }
}

export async function trashWorkspaceEntry(path: string): Promise<boolean> {
  const root = activeRoot.value
  if (!root || !isTauri()) return false
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
    await loadDocuments(root)
    return true
  } catch (reason) {
    setError(reason)
    return false
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

export async function emptyWorkspaceTrash() {
  if (!isTauri() || !window.confirm(i18n.global.t('app.emptyTrashConfirm'))) return
  try {
    await nativeApi.emptyWorkspaceTrash()
    trashEntries.value = []
  } catch (reason) {
    setError(reason)
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
  if (imagePreview.value?.root === root) {
    const nextPath = byOldPath.get(imagePreview.value.path)
    if (nextPath) imagePreview.value.path = nextPath
  }
}

function pathContains(parent: string, candidate: string) {
  return candidate === parent || candidate.startsWith(`${parent}/`)
}
