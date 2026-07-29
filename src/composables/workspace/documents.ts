import { i18n } from '@/i18n'
import { isTauri, nativeApi } from '@/lib/api'
import {
  activeRepository,
  activeRoot,
  activeTab,
  beginLoading,
  endLoading,
  ensureSession,
  fileName,
  imagePreview,
  message,
  recentFiles,
  setError,
  tabKey,
  updateWorktreeStatus,
} from './state'
import { reconcileOpenTabs, scheduleSave } from './persistence'
import { demoContents } from './demoData'

export async function loadDocuments(requestedRoot = activeRoot.value) {
  const root = requestedRoot
  if (!root || !isTauri()) return
  const session = ensureSession(root)
  const generation = ++session.loadGeneration
  beginLoading()
  try {
    const [status, branchList, pendingOperation] = await Promise.all([
      nativeApi.repositoryStatus({ root }),
      nativeApi.listBranches({ root }),
      nativeApi.pendingGitOperation({ root }),
    ])
    const pendingConflicts = await nativeApi.pendingConflicts({ root })
    const nextDocuments = await nativeApi.listDocuments({ root, statuses: status.files })
    if (session.loadGeneration !== generation) return
    if (!pendingOperation && !pendingConflicts.length) {
      await reconcileOpenTabs(session, nextDocuments, generation)
      if (session.loadGeneration !== generation) return
    }
    updateWorktreeStatus(root, status)
    session.branches = branchList
    session.conflicts = pendingConflicts
    session.pendingOperation = pendingOperation ?? undefined
    session.documents = nextDocuments
    if (pendingOperation && !pendingConflicts.length) {
      message.value = i18n.global.t('app.pendingGitOperation')
    }
  } catch (reason) {
    setError(reason)
  } finally {
    endLoading()
  }
}

export async function loadRepositoryImage(root: string, path: string) {
  const preview = await nativeApi.readAsset({ root, path })
  return `data:${preview.mediaType};base64,${preview.base64Data}`
}

export async function openDocument(path: string) {
  const root = activeRoot.value
  if (!root) return
  const descriptor = ensureSession(root).documents.find(
    (document) => document.path === path,
  )
  if (descriptor?.kind === 'image') {
    try {
      imagePreview.value = {
        root,
        path,
        url: await loadRepositoryImage(root, path),
      }
    } catch (reason) {
      setError(reason)
    }
    return
  }
  if (descriptor?.kind === 'other') {
    setError(i18n.global.t('app.unsupportedPreview'))
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
    const matches = await nativeApi.searchWorktrees({
      root: activeRepository.value?.root ?? root,
      query,
      limit: 120,
    })
    if (session.searchGeneration !== generation || session.searchQuery !== query) return
    session.crossWorktreeMatches = matches
    session.searchMatches = matches
      .filter((match) => match.root === root)
      .map((match) => match.path)
  } catch (reason) {
    setError(reason)
  }
}

export async function handleRepositoryChanged(root: string) {
  await loadDocuments(root)
}
