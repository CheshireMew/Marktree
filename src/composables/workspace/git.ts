import { i18n } from '@/i18n'
import { isTauri, nativeApi } from '@/lib/api'
import { createTextDiffResult } from '@/lib/textDiff'
import type { ConflictRecord, DiffMode } from '@/types'

import {
  activeRoot,
  activeStatus,
  activeTab,
  activeWorkspace,
  diffOpen,
  diffResult,
  ensureSession,
  message,
  setError,
  syncing,
  updateWorktreeStatus,
} from './state'
import { flushAll } from './persistence'
import { refreshActive } from './workspaces'

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
    diffResult.value = createTextDiffResult({
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

export function showUnsavedDiff() {
  const tab = activeTab.value
  if (!tab) return
  diffResult.value = createTextDiffResult({
    mode: 'unsavedToDisk',
    oldLabel: i18n.global.t('app.disk'),
    newLabel: i18n.global.t('app.editor'),
    path: tab.path,
    header: i18n.global.t('app.diffUnsavedDisk'),
    oldText: tab.diskContent,
    newText: tab.content,
  })
  diffOpen.value = true
}

export async function sync() {
  const root = activeRoot.value
  if (!root || !activeWorkspace.value?.git || !isTauri() || syncing.value) return
  syncing.value = true
  try {
    await flushAll(root)
    const pending = await nativeApi.pendingGitOperation({ root })
    let result
    if (pending) {
      if (pending.aborting) {
        await nativeApi.abortGitOperation({ root })
        ensureSession(root).pendingOperation = undefined
        message.value = i18n.global.t('app.gitOperationAborted')
        await refreshActive()
        return
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
      message.value = i18n.global.t('app.syncComplete')
      await refreshActive()
    }
  } catch (reason) {
    setError(reason)
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
    message.value = i18n.global.t('app.syncComplete')
    await refreshActive()
  }
}

export async function abortGitOperation() {
  const root = activeRoot.value
  if (!root || !isTauri() || syncing.value) return
  syncing.value = true
  try {
    await nativeApi.abortGitOperation({ root })
    const session = ensureSession(root)
    session.conflicts = []
    session.pendingOperation = undefined
    message.value = i18n.global.t('app.gitOperationAborted')
    await refreshActive()
  } catch (reason) {
    setError(reason)
  } finally {
    syncing.value = false
  }
}

export async function gitAction(
  action: 'fetch' | 'pull' | 'push' | 'stageAll' | 'commit',
  payload?: string,
) {
  const root = activeRoot.value
  if (!root) return
  try {
    if (action !== 'fetch' && action !== 'push') await flushAll(root)
    if (action === 'fetch') await nativeApi.fetch({ root })
    if (action === 'pull') {
      const result = await nativeApi.pullRebase({ root })
      throwIfSyncFailed(result)
      const session = ensureSession(root)
      session.conflicts = result.conflicts
      session.pendingOperation = (await nativeApi.pendingGitOperation({ root })) ?? undefined
    }
    if (action === 'push') await nativeApi.push({ root })
    if (action === 'stageAll') await nativeApi.stageAll({ root })
    if (action === 'commit') await nativeApi.commit({ root, message: payload ?? '' })
    await refreshActive()
  } catch (reason) {
    setError(reason)
    if (action === 'pull') {
      try {
        ensureSession(root).pendingOperation =
          (await nativeApi.pendingGitOperation({ root })) ?? undefined
      } catch {
        // Preserve the primary Git error.
      }
    }
  }
}

export async function setPathStaged(path: string, staged: boolean) {
  const root = activeRoot.value
  if (!root) return
  try {
    await flushAll(root)
    const status = staged
      ? await nativeApi.stagePaths({ root, paths: [path] })
      : await nativeApi.unstagePaths({ root, paths: [path] })
    updateWorktreeStatus(root, status)
  } catch (reason) {
    setError(reason)
  }
}

export async function createBranch(name: string, startPoint?: string) {
  const root = activeRoot.value
  if (!root || !name.trim()) return
  try {
    await flushAll(root)
    const status = await nativeApi.createBranch({
      root,
      name: name.trim(),
      startPoint: startPoint?.trim() || null,
      checkout: true,
    })
    updateWorktreeStatus(root, status)
    await refreshActive()
  } catch (reason) {
    setError(reason)
  }
}

export async function checkoutBranch(name: string) {
  const root = activeRoot.value
  if (!root || activeStatus.value?.branch === name) return
  try {
    await flushAll(root)
    const status = await nativeApi.checkoutBranch({ root, name })
    updateWorktreeStatus(root, status)
    await refreshActive()
  } catch (reason) {
    setError(reason)
  }
}

export async function deleteBranch(name: string) {
  const root = activeRoot.value
  if (!root) return
  try {
    await flushAll(root)
    ensureSession(root).branches = await nativeApi.deleteBranch({ root, name })
    await refreshActive()
  } catch (reason) {
    setError(reason)
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
