import { i18n } from '@/i18n'
import { isTauri, nativeApi } from '@/lib/api'
import {
  removeWorkspaceUiState,
  restoredWorkspaceSession,
} from '@/lib/workspaceUiState'
import type { WorkspaceDescriptor, WorktreeDescriptor, WorktreeSearchResult } from '@/types'

import {
  activeWorkspace,
  activeWorkspaceId,
  activeRoot,
  activeWorktreePath,
  addOrReplaceWorkspace,
  beginLoading,
  endLoading,
  ensureSession,
  recentFiles,
  recentFileLimit,
  sessions,
  workspaces,
  setError,
  setNotice,
} from './state'
import { loadDocuments, openDocument } from './documents'
import { disposeSession, flushAll } from './persistence'
import { loadDemoWorkspace } from './demo'

const restoredRoots = new Set<string>()

export async function initializeWorkspace() {
  if (!isTauri()) {
    const demo = new URLSearchParams(location.search).get('demo')
    if (demo) loadDemoWorkspace(demo === 'git')
    return
  }
  beginLoading()
  try {
    const state = await nativeApi.getStartupState()
    recentFiles.value = state.recentFiles
    recentFileLimit.value = state.recentFileLimit
    const queryRoot = new URLSearchParams(location.search).get('root')
    const queryWorktree = new URLSearchParams(location.search).get('worktree')
    const roots = [...state.workspaces]
    if (queryRoot && !roots.includes(queryRoot)) roots.push(queryRoot)
    const restored = await Promise.allSettled(
      roots.map((root) => nativeApi.openWorkspace({ path: root })),
    )
    for (const result of restored) {
      if (result.status === 'fulfilled') addOrReplaceWorkspace(result.value)
      // Moved or temporarily unavailable workspaces remain in local state.
    }
    const queryIndex = queryRoot ? roots.indexOf(queryRoot) : -1
    const queriedResult = queryIndex >= 0 ? restored[queryIndex] : undefined
    const queried = queriedResult?.status === 'fulfilled' ? queriedResult.value : undefined
    if (queried) {
      await activateWorkspace(queried.id, queryWorktree ?? queryRoot ?? undefined, true)
    } else if (workspaces.value[0]) {
      await activateWorkspace(workspaces.value[0].id, undefined, true)
    }
  } catch (reason) {
    setError(reason)
  } finally {
    endLoading()
  }
}

export async function addWorkspace(descriptor: WorkspaceDescriptor) {
  addOrReplaceWorkspace(descriptor)
  await activateWorkspace(descriptor.id)
}

export async function forgetActiveWorkspace() {
  const workspace = activeWorkspace.value
  const root = activeRoot.value
  if (!workspace || !root || !isTauri()) return
  const roots = workspace.git?.worktrees.map((worktree) => worktree.path) ?? [workspace.root]
  for (const contentRoot of roots) {
    await flushAll(contentRoot)
  }
  await nativeApi.forgetWorkspace({ root: workspace.root })
  await removeWorkspace(workspace)
}

export async function handleWorkspaceForgotten(workspaceId: string, roots: string[]) {
  const workspace =
    workspaces.value.find((candidate) => candidate.id === workspaceId) ??
    workspaces.value.find((candidate) =>
      (candidate.git?.worktrees.map((worktree) => worktree.path) ?? [candidate.root])
        .some((root) => roots.includes(root)),
    )
  if (workspace) await removeWorkspace(workspace)
}

export async function removeWorkspace(workspace: WorkspaceDescriptor) {
  if (!workspaces.value.some((candidate) => candidate.id === workspace.id)) return
  const roots = new Set(
    workspace.git?.worktrees.map((worktree) => worktree.path) ?? [workspace.root],
  )
  for (const sessionRoot of roots) {
    disposeSession(sessionRoot)
    restoredRoots.delete(sessionRoot)
  }
  removeWorkspaceUiState(roots)
  workspaces.value = workspaces.value.filter(
    (candidate) => candidate.id !== workspace.id,
  )
  activeWorkspaceId.value = undefined
  activeWorktreePath.value = undefined
  setNotice(i18n.global.t('app.workspaceForgotten'))
  const next = workspaces.value[0]
  if (next) await activateWorkspace(next.id)
}

export async function activateWorkspace(
  id: string,
  preferredWorktree?: string,
  lazyTabRestore = false,
) {
  const workspace = workspaces.value.find((item) => item.id === id)
  if (!workspace) return
  activeWorkspaceId.value = id
  const worktrees = workspace.git?.worktrees ?? []
  const path =
    preferredWorktree && worktrees.some((item) => item.path === preferredWorktree)
      ? preferredWorktree
      : worktrees.find((item) => item.path === workspace.root)?.path ??
        worktrees[0]?.path ??
        workspace.root
  activeWorktreePath.value = path
  ensureSession(path)
  await loadDocuments(path)
  await restoreWorkspaceTabs(path, lazyTabRestore)
}

export async function selectWorktree(worktree: WorktreeDescriptor) {
  activeWorktreePath.value = worktree.path
  ensureSession(worktree.path)
  await loadDocuments(worktree.path)
  await restoreWorkspaceTabs(worktree.path)
}

export async function refreshActive(fetchRemote = false, reportError = true) {
  const root = activeRoot.value
  if (!root || !isTauri()) return
  try {
    if (fetchRemote && activeWorkspace.value?.git?.remoteUrl) {
      await nativeApi.fetch({ root })
    }
    const workspaceRoot = activeWorkspace.value?.root ?? root
    const snapshot = await nativeApi.refreshWorkspaceView({
      workspaceRoot,
      contentRoot: root,
    })
    addOrReplaceWorkspace(snapshot.workspace)
    await loadDocuments(root, snapshot.view)
  } catch (reason) {
    if (reportError) setError(reason)
    else throw reason
  }
}

export async function previewWorkspaceGitBaseline(): Promise<string | undefined> {
  const workspace = activeWorkspace.value
  if (!workspace || workspace.git || !isTauri()) return undefined
  const preview = await nativeApi.previewWorkspaceGitBaseline({
    root: workspace.root,
  })
  return i18n.global.t('app.enableGitConfirm', {
    count: preview.fileCount,
    size: formatBytes(preview.totalBytes),
    ignored: preview.ignoredCount,
  })
}

export async function enableWorkspaceGit(): Promise<boolean> {
  const workspace = activeWorkspace.value
  if (!workspace || workspace.git || !isTauri()) return false
  try {
    beginLoading()
    const descriptor = await nativeApi.enableWorkspaceGit({ root: workspace.root })
    addOrReplaceWorkspace(descriptor)
    await activateWorkspace(descriptor.id)
    setNotice(i18n.global.t('app.gitEnabled'))
    return true
  } catch (reason) {
    setError(reason)
    return false
  } finally {
    endLoading()
  }
}

export async function openSearchResult(result: WorktreeSearchResult) {
  const owner = workspaces.value.find(
    (candidate) =>
      candidate.root === result.root ||
      candidate.git?.worktrees.some((worktree) => worktree.path === result.root),
  )
  if (owner && owner.id !== activeWorkspace.value?.id) {
    await activateWorkspace(owner.id, result.root)
  } else {
    const worktree = owner?.git?.worktrees.find(
      (candidate) => candidate.path === result.root,
    )
    if (worktree && activeRoot.value !== worktree.path) await selectWorktree(worktree)
  }
  await openDocument(result.path)
}

async function restoreWorkspaceTabs(root: string, lazy = false) {
  if (restoredRoots.has(root)) return
  restoredRoots.add(root)
  const restored = restoredWorkspaceSession(root)
  if (!restored) return
  const session = ensureSession(root)
  const available = new Set(
    session.entries
      .filter(
        (entry) =>
          entry.entryType === 'file' &&
          (entry.fileKind === 'markdown' || entry.fileKind === 'text'),
      )
      .map((entry) => entry.path),
  )
  const paths = restored.tabs.filter((candidate) => available.has(candidate))
  if (!lazy) {
    for (const path of paths) await openDocument(path)
    if (restored.active && session.tabs.some((tab) => tab.path === restored.active)) {
      session.activeTabKey = `${root}\n${restored.active}`
    }
    return
  }
  const active = restored.active && paths.includes(restored.active)
    ? restored.active
    : paths[0]
  if (!active) return
  await openDocument(active)
  const remaining = paths.filter((path) => path !== active)
  void restoreTabsInBackground(root, remaining)
}

async function restoreTabsInBackground(root: string, paths: string[]) {
  let cursor = 0
  const worker = async () => {
    while (cursor < paths.length && sessions.has(root)) {
      const path = paths[cursor]
      cursor += 1
      if (path) await openDocument(path, { activate: false, remember: false, root })
    }
  }
  await Promise.all([worker(), worker()])
}

function formatBytes(bytes: number) {
  if (bytes < 1024) return `${bytes} B`
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`
  return `${(bytes / 1024 / 1024).toFixed(1)} MB`
}
