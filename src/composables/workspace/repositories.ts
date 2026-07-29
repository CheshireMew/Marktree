import { i18n } from '@/i18n'
import { isTauri, nativeApi } from '@/lib/api'
import type { RepositoryDescriptor, WorktreeDescriptor, WorktreeSearchResult } from '@/types'

import {
  activeRepository,
  activeRepositoryId,
  activeRoot,
  activeWorktreePath,
  addOrReplaceRepository,
  beginLoading,
  endLoading,
  ensureSession,
  message,
  recentFiles,
  repositories,
  setError,
  updateWorktreeStatus,
} from './state'
import { loadDocuments, openDocument } from './documents'
import { disposeSession, flushAll } from './persistence'
import { loadDemoWorkspace } from './demo'

export async function initializeWorkspace() {
  if (!isTauri()) {
    if (new URLSearchParams(location.search).has('demo')) loadDemoWorkspace()
    return
  }
  beginLoading()
  try {
    const state = await nativeApi.getLocalState()
    recentFiles.value = state.recentFiles
    for (const root of state.repositories) {
      try {
        const descriptor = await nativeApi.openRepository({ path: root })
        addOrReplaceRepository(descriptor)
      } catch {
        // A moved or temporarily unavailable repository remains in local state.
      }
    }
    const queryRoot = new URLSearchParams(location.search).get('root')
    if (queryRoot) {
      const descriptor = await nativeApi.openRepository({ path: queryRoot })
      addOrReplaceRepository(descriptor)
      await activateRepository(descriptor.id, queryRoot)
    } else if (repositories.value[0]) {
      await activateRepository(repositories.value[0].id)
    }
  } catch (reason) {
    setError(reason)
  } finally {
    endLoading()
  }
}

export async function addRepository(descriptor: RepositoryDescriptor) {
  addOrReplaceRepository(descriptor)
  await activateRepository(descriptor.id)
}

export async function forgetActiveRepository() {
  const repository = activeRepository.value
  const root = activeRoot.value
  if (!repository || !root || !isTauri()) return
  for (const worktree of repository.worktrees) {
    await flushAll(worktree.path)
  }
  await nativeApi.forgetRepository({ root })
  await removeRepository(repository)
}

export async function handleRepositoryForgotten(repositoryId: string, worktreeRoots: string[]) {
  const repository =
    repositories.value.find((candidate) => candidate.id === repositoryId) ??
    repositories.value.find((candidate) =>
      candidate.worktrees.some((worktree) => worktreeRoots.includes(worktree.path)),
    )
  if (repository) await removeRepository(repository)
}

export async function removeRepository(repository: RepositoryDescriptor) {
  if (!repositories.value.some((candidate) => candidate.id === repository.id)) return
  const roots = new Set(repository.worktrees.map((worktree) => worktree.path))
  for (const sessionRoot of roots) {
    disposeSession(sessionRoot)
  }
  repositories.value = repositories.value.filter(
    (candidate) => candidate.id !== repository.id,
  )
  activeRepositoryId.value = undefined
  activeWorktreePath.value = undefined
  message.value = i18n.global.t('app.repositoryForgotten')
  const next = repositories.value[0]
  if (next) await activateRepository(next.id)
}

export async function activateRepository(id: string, preferredWorktree?: string) {
  const repository = repositories.value.find((item) => item.id === id)
  if (!repository) return
  activeRepositoryId.value = id
  const path =
    preferredWorktree && repository.worktrees.some((item) => item.path === preferredWorktree)
      ? preferredWorktree
      : repository.worktrees[0]?.path
  activeWorktreePath.value = path
  if (path) ensureSession(path)
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
    if (fetchRemote && activeRepository.value?.remoteUrl) {
      const status = await nativeApi.fetch({ root })
      updateWorktreeStatus(root, status)
    }
    const descriptor = await nativeApi.refreshRepository({ root })
    addOrReplaceRepository(descriptor)
    await loadDocuments(root)
  } catch (reason) {
    if (reportError) setError(reason)
    else throw reason
  }
}

export async function openSearchResult(result: WorktreeSearchResult) {
  const worktree = activeRepository.value?.worktrees.find(
    (candidate) => candidate.path === result.root,
  )
  if (worktree) await selectWorktree(worktree)
  await openDocument(result.path)
}
