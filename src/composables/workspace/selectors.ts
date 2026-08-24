import { computed } from 'vue'

import {
  favoriteDocumentKeys,
  isFavoriteDocument,
  toggleFavoriteDocument,
} from '@/lib/workspaceUiState'
import { editableDocumentCharacterLimit } from '@/lib/documentMemory'
import type { GitStatusSnapshot, WorkspaceEntry, WorktreeSearchResult } from '@/types'

import {
  activeWorkspaceId,
  activeWorktreePath,
  persistSessionPaths,
  recentFiles,
  retainedEditorTabs,
  sessions,
  tabKey,
  workspaces,
} from './store'

export const activeWorkspace = computed(() =>
  workspaces.value.find((workspace) => workspace.id === activeWorkspaceId.value),
)

export const activeWorktree = computed(() => {
  const capability = activeWorkspace.value?.git
  if (!capability) return undefined
  return (
    capability.worktrees.find((worktree) => worktree.path === activeWorktreePath.value) ??
    capability.worktrees[0]
  )
})

export const activeRoot = computed(
  () => activeWorktree.value?.path ?? activeWorkspace.value?.root,
)
export const activeStatus = computed<GitStatusSnapshot | undefined>(
  () => activeWorktree.value?.status ?? undefined,
)
export const activeSession = computed(() => {
  const root = activeRoot.value
  return root ? sessions.get(root) : undefined
})
export const entries = computed(() => activeSession.value?.entries ?? [])
export const documents = computed(() =>
  entries.value.filter((entry) => entry.entryType === 'file'),
)
export const branches = computed(() => activeSession.value?.branches ?? [])
export const tabs = computed(() => activeSession.value?.tabs ?? [])
export const activeTabKey = computed<string | undefined>({
  get: () => activeSession.value?.activeTabKey,
  set: (value) => {
    const session = activeSession.value
    if (session) {
      session.activeTabKey = value
      persistSessionPaths(session)
    }
  },
})
export const activeTab = computed(() => {
  const session = activeSession.value
  return session?.tabs.find((tab) => tabKey(tab.root, tab.path) === session.activeTabKey)
})
export const activeDocumentCharacterLimit = computed(() =>
  editableDocumentCharacterLimit(retainedEditorTabs(), activeTab.value),
)
export const searchQuery = computed<string>({
  get: () => activeSession.value?.searchQuery ?? '',
  set: (value) => {
    const session = activeSession.value
    if (session) session.searchQuery = value
  },
})
export const crossWorktreeMatches = computed(
  () => activeSession.value?.crossWorktreeMatches ?? [],
)
export const searchResults = computed<WorktreeSearchResult[]>(() => {
  const session = activeSession.value
  const root = activeRoot.value
  const query = session?.searchQuery.trim().toLocaleLowerCase() ?? ''
  if (!session || !root || !query) return []
  const ownerName = activeWorktree.value?.name ?? activeWorkspace.value?.name ?? ''
  const results = session.searchMatches.map((result) => ({
    worktree: ownerName,
    root,
    ...result,
  }))
  const seen = new Set(results.map((result) => `${result.path}\n${result.line ?? 0}`))
  for (const entry of session.entries) {
    if (
      entry.entryType !== 'file' ||
      !entry.path.toLocaleLowerCase().includes(query) ||
      seen.has(`${entry.path}\n0`)
    ) continue
    results.push({
      worktree: ownerName,
      root,
      path: entry.path,
      line: null,
      column: null,
      snippet: entry.path,
      matchType: 'path',
      fileKind: entry.fileKind ?? 'other',
      modifiedMs: entry.modifiedMs,
    })
  }
  return results
})
export const searchInProgress = computed(() => activeSession.value?.searching ?? false)
export const conflicts = computed(() => activeSession.value?.conflicts ?? [])
export const pendingOperation = computed(() => activeSession.value?.pendingOperation)

export const recentDocuments = computed(() => {
  const root = activeRoot.value
  if (!root) return documents.value
  const ranks = new Map(recentFiles.value.map((key, index) => [key, index]))
  return [...documents.value].sort((left, right) => {
    const leftRank = ranks.get(`${root}\n${left.path}`) ?? Number.MAX_SAFE_INTEGER
    const rightRank = ranks.get(`${root}\n${right.path}`) ?? Number.MAX_SAFE_INTEGER
    return leftRank - rightRank || left.path.localeCompare(right.path)
  })
})

export const favoriteDocuments = computed(() => {
  const root = activeRoot.value
  if (!root) return []
  const byPath = new Map(documents.value.map((entry) => [entry.path, entry]))
  return favoriteDocumentKeys.value
    .map((key) => {
      const separator = key.indexOf('\n')
      if (separator < 0 || key.slice(0, separator) !== root) return undefined
      return byPath.get(key.slice(separator + 1))
    })
    .filter((entry): entry is WorkspaceEntry => Boolean(entry))
})

export function activeDocumentIsFavorite() {
  const tab = activeTab.value
  return tab ? isFavoriteDocument(tab.root, tab.path) : false
}

export function toggleActiveDocumentFavorite() {
  const tab = activeTab.value
  if (tab) toggleFavoriteDocument(tab.root, tab.path)
}

export function toggleFavoritePath(path: string) {
  const root = activeRoot.value
  if (root) toggleFavoriteDocument(root, path)
}

export function selectTab(key: string | undefined) {
  activeTabKey.value = key
}

export function setSearchQuery(value: string) {
  const session = activeSession.value
  if (!session) return
  session.searchQuery = value
  session.searchGeneration += 1
  session.searchMatches = []
  session.crossWorktreeMatches = []
  session.searching = Boolean(value.trim())
}
