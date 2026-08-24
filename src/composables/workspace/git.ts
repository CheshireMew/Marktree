import { i18n } from '@/i18n'
import { isTauri, nativeApi } from '@/lib/api'
import { createTextDiffResultAsync } from '@/lib/textDiff'
import type { ConflictRecord, DiffMode, WorkspaceViewSnapshot } from '@/types'

import {
  activeRoot,
  activeStatus,
  activeTab,
  activeWorkspace,
  clearNotice,
  diffOpen,
  diffResult,
  ensureSession,
  gitBusyAction,
  setError,
  setNotice,
  syncing,
} from './state'
import { flushAll } from './persistence'
import { refreshActive } from './workspaces'
import { loadDocuments } from './documents'

export async function showDiff(mode: DiffMode) {
  const root = activeRoot.value
  if (!root || !isTauri()) return
  try {
    diffResult.value = await nativeApi.gitDiff({ root, mode })
    diffOpen.value = true
  } catch (reason) {
    setError(reason)
  }
}

export async function showWorktreeDiff(rightRoot: string) {
  const tab = activeTab.value
  const leftRoot = activeRoot.value
  if (!tab || !leftRoot || !rightRoot || !isTauri()) return
  try {
    const comparison = await nativeApi.compareWorktrees({
      leftRoot,
      rightRoot,
      path: tab.path,
    })
    diffResult.value = await createTextDiffResultAsync({
      mode: 'worktreeToWorktree',
      oldLabel: comparison.leftLabel,
      newLabel: comparison.rightLabel,
      path: comparison.path,
      header: `${comparison.leftLabel} ↔ ${comparison.rightLabel}`,
      oldText: comparison.left,
      newText: comparison.right,
    })
    diffOpen.value = true
  } catch (reason) {
    setError(reason)
  }
}

export async function showUnsavedDiff() {
  const tab = activeTab.value
  if (!tab) return
  try {
    const diskContent = tab.diskContent ?? (
      isTauri()
        ? (await nativeApi.readDocument({ root: tab.root, path: tab.path })).content
        : tab.content
    )
    diffResult.value = await createTextDiffResultAsync({
      mode: 'unsavedToDisk',
      oldLabel: i18n.global.t('app.disk'),
      newLabel: i18n.global.t('app.editor'),
      path: tab.path,
      header: i18n.global.t('app.diffUnsavedDisk'),
      oldText: diskContent,
      newText: tab.content,
    })
    diffOpen.value = true
  } catch (reason) {
    setError(reason)
  }
}

export async function sync(): Promise<boolean> {
  const root = activeRoot.value
  if (
    !root ||
    !activeWorkspace.value?.git ||
    !isTauri() ||
    syncing.value ||
    gitBusyAction.value
  ) return false
  syncing.value = true
  clearNotice()
  try {
    await flushAll(root)
    const pending = await nativeApi.pendingGitOperation({ root })
    let result
    if (pending) {
      if (pending.aborting) {
        await nativeApi.abortGitOperation({ root })
        ensureSession(root).pendingOperation = undefined
        setNotice(i18n.global.t('app.gitOperationAborted'))
        await refreshActive()
        return true
      }
      result = await nativeApi.resumeGitOperation({ root })
    } else {
      const plan = await nativeApi.syncPlan({ root })
      if (!plan.canPush) {
        throw new Error(i18n.global.t('app.syncUnavailable'))
      }
      result = await nativeApi.syncWorkspaceChanges({ root })
    }
    throwIfSyncFailed(result)
    const session = ensureSession(root)
    session.conflicts = result.conflicts
    session.pendingOperation = (await nativeApi.pendingGitOperation({ root })) ?? undefined
    if (!result.conflicts.length) {
      setNotice(i18n.global.t('app.syncComplete'))
      await refreshActive()
    }
    return true
  } catch (reason) {
    setError(reason)
    return false
  } finally {
    try {
      ensureSession(root).pendingOperation =
        (await nativeApi.pendingGitOperation({ root })) ?? undefined
    } catch {
      // Preserve the primary operation error; a normal refresh will retry this status read.
    }
    syncing.value = false
  }
}

export async function resolveConflict(conflict: ConflictRecord, choice: 'local' | 'remote') {
  const root = activeRoot.value
  if (!root) return
  try {
    await nativeApi.resolveConflict({
      root,
      path: conflict.path,
      recoveryId: conflict.recoveryId,
      choice,
    })
    await finishConflictResolution(conflict.recoveryId)
  } catch (reason) {
    setError(reason)
  }
}

export async function resolveConflictContent(conflict: ConflictRecord, content: string) {
  const root = activeRoot.value
  if (!root) return
  try {
    await nativeApi.resolveConflictWithContent({
      root,
      path: conflict.path,
      recoveryId: conflict.recoveryId,
      content,
    })
    await finishConflictResolution(conflict.recoveryId)
  } catch (reason) {
    setError(reason)
  }
}

export async function finishConflictResolution(recoveryId: string) {
  const root = activeRoot.value
  if (!root) return
  const session = ensureSession(root)
  session.conflicts = session.conflicts.filter((item) => item.recoveryId !== recoveryId)
  if (session.conflicts.length) return
  const result = await nativeApi.resumeGitOperation({ root })
  throwIfSyncFailed(result)
  session.conflicts = result.conflicts
  session.pendingOperation = (await nativeApi.pendingGitOperation({ root })) ?? undefined
  if (!result.conflicts.length) {
    setNotice(i18n.global.t('app.syncComplete'))
    await refreshActive()
  }
}

export async function abortGitOperation(): Promise<boolean> {
  const root = activeRoot.value
  if (!root || !isTauri() || syncing.value || gitBusyAction.value) return false
  syncing.value = true
  clearNotice()
  try {
    await nativeApi.abortGitOperation({ root })
    const session = ensureSession(root)
    session.conflicts = []
    session.pendingOperation = undefined
    setNotice(i18n.global.t('app.gitOperationAborted'))
    await refreshActive()
    return true
  } catch (reason) {
    setError(reason)
    return false
  } finally {
    syncing.value = false
  }
}

export async function gitAction(
  action: 'fetch' | 'pull' | 'push' | 'stageAll' | 'commit',
  payload?: string,
): Promise<boolean> {
  return runGitOperation(action, async (root) => {
    if (action !== 'fetch' && action !== 'push') await flushAll(root)
    let view: WorkspaceViewSnapshot | undefined
    if (action === 'fetch') await nativeApi.fetch({ root })
    if (action === 'pull') {
      const result = await nativeApi.pullRebase({ root })
      throwIfSyncFailed(result)
      const session = ensureSession(root)
      session.conflicts = result.conflicts
      session.pendingOperation = (await nativeApi.pendingGitOperation({ root })) ?? undefined
    }
    if (action === 'push') await nativeApi.push({ root })
    if (action === 'stageAll') view = await nativeApi.stageAll({ root })
    if (action === 'commit') view = await nativeApi.commit({ root, message: payload ?? '' })
    if (view) await loadDocuments(root, view)
    else await refreshActive()
  }, async (root) => {
    if (action !== 'pull') return
    try {
      ensureSession(root).pendingOperation =
        (await nativeApi.pendingGitOperation({ root })) ?? undefined
    } catch {
      // Preserve the primary Git error.
    }
  })
}

export async function setPathStaged(path: string, staged: boolean): Promise<boolean> {
  return runGitOperation(staged ? 'stagePath' : 'unstagePath', async (root) => {
    await flushAll(root)
    const view = staged
      ? await nativeApi.stagePaths({ root, paths: [path] })
      : await nativeApi.unstagePaths({ root, paths: [path] })
    await loadDocuments(root, view)
  })
}

export async function createBranch(name: string, startPoint?: string): Promise<boolean> {
  if (!name.trim()) return false
  return runGitOperation('createBranch', async (root) => {
    await flushAll(root)
    const view = await nativeApi.createBranch({
      root,
      name: name.trim(),
      startPoint: startPoint?.trim() || null,
      checkout: true,
    })
    await loadDocuments(root, view)
  })
}

export async function checkoutBranch(name: string): Promise<boolean> {
  if (activeStatus.value?.branch === name) return true
  return runGitOperation('checkoutBranch', async (root) => {
    await flushAll(root)
    const view = await nativeApi.checkoutBranch({ root, name })
    await loadDocuments(root, view)
  })
}

export async function deleteBranch(name: string): Promise<boolean> {
  return runGitOperation('deleteBranch', async (root) => {
    await flushAll(root)
    ensureSession(root).branches = await nativeApi.deleteBranch({ root, name })
  })
}

async function runGitOperation(
  action: string,
  operation: (root: string) => Promise<void>,
  afterFailure?: (root: string) => Promise<void>,
): Promise<boolean> {
  const root = activeRoot.value
  if (!root || gitBusyAction.value || syncing.value) return false
  gitBusyAction.value = action
  clearNotice()
  try {
    await operation(root)
    return true
  } catch (reason) {
    setError(reason)
    await afterFailure?.(root)
    return false
  } finally {
    gitBusyAction.value = undefined
  }
}

export function throwIfSyncFailed(result: {
  failureStage: string | null
  error: { message: string } | null
}) {
  if (!result.error) return
  const stage = result.failureStage ? ` (${result.failureStage})` : ''
  throw new Error(`${result.error.message}${stage}`)
}
