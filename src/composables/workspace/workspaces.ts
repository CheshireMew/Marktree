import { i18n } from '@/i18n'
import { isTauri, nativeApi } from '@/lib/api'
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
  message,
  recentFiles,
  workspaces,
  setError,
  updateWorktreeStatus,
} from './state'
import { loadDocuments, openDocument } from './documents'
import { disposeSession, flushAll } from './persistence'
import { loadDemoWorkspace } from './demo'

export async function initializeWorkspace() {
  if (!isTauri()) {
    const demo = new URLSearchParams(location.search).get('demo')
    if (demo) loadDemoWorkspace(demo === 'git')
    return
  }
  beginLoading()
  try {
    const state = await nativeApi.getLocalState()
    recentFiles.value = state.recentFiles
    for (const root of state.workspaces) {
      try {
        const descriptor = await nativeApi.openWorkspace({ path: root })
        addOrReplaceWorkspace(descriptor)
      } catch {
        // A moved or temporarily unavailable workspace remains in local state.
      }
    }
    const queryRoot = new URLSearchParams(location.search).get('root')
    const queryWorktree = new URLSearchParams(location.search).get('worktree')
    if (queryRoot) {
      const descriptor = await nativeApi.openWorkspace({ path: queryRoot })
      addOrReplaceWorkspace(descriptor)
      await activateWorkspace(descriptor.id, queryWorktree ?? queryRoot)
    } else if (workspaces.value[0]) {
      await activateWorkspace(workspaces.value[0].id)
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
  }
  workspaces.value = workspaces.value.filter(
    (candidate) => candidate.id !== workspace.id,
  )
  activeWorkspaceId.value = undefined
  activeWorktreePath.value = undefined
  message.value = i18n.global.t('app.workspaceForgotten')
  const next = workspaces.value[0]
  if (next) await activateWorkspace(next.id)
}

export async function activateWorkspace(id: string, preferredWorktree?: string) {
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
}

export async function selectWorktree(worktree: WorktreeDescriptor) {
  activeWorktreePath.value = worktree.path
  ensureSession(worktree.path)
  await loadDocuments(worktree.path)
}

export async function refreshActive(fetchRemote = false, reportError = true) {
  const root = activeRoot.value
  if (!root || !isTauri()) return
  try {
    if (fetchRemote && activeWorkspace.value?.git?.remoteUrl) {
      const status = await nativeApi.fetch({ root })
      updateWorktreeStatus(root, status)
    }
    const workspaceRoot = activeWorkspace.value?.root ?? root
    const descriptor = await nativeApi.refreshWorkspace({ root: workspaceRoot })
    addOrReplaceWorkspace(descriptor)
    await loadDocuments(root)
  } catch (reason) {
    if (reportError) setError(reason)
    else throw reason
  }
}

export async function enableWorkspaceGit() {
  const workspace = activeWorkspace.value
  if (!workspace || workspace.git || !isTauri()) return
  try {
    const preview = await nativeApi.previewWorkspaceGitBaseline({
      root: workspace.root,
    })
    const confirmed = window.confirm(
      i18n.global.t('app.enableGitConfirm', {
        count: preview.fileCount,
        size: formatBytes(preview.totalBytes),
        ignored: preview.ignoredCount,
      }),
    )
    if (!confirmed) return
    beginLoading()
    const descriptor = await nativeApi.enableWorkspaceGit({ root: workspace.root })
    addOrReplaceWorkspace(descriptor)
    await activateWorkspace(descriptor.id)
    message.value = i18n.global.t('app.gitEnabled')
  } catch (reason) {
    setError(reason)
  } finally {
    endLoading()
  }
}

export async function openSearchResult(result: WorktreeSearchResult) {
  const worktree = activeWorkspace.value?.git?.worktrees.find(
    (candidate) => candidate.path === result.root,
  )
  if (worktree) await selectWorktree(worktree)
  await openDocument(result.path)
}

function formatBytes(bytes: number) {
  if (bytes < 1024) return `${bytes} B`
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`
  return `${(bytes / 1024 / 1024).toFixed(1)} MB`
}
