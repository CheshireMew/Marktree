import { isTauri, nativeApi } from '@/lib/api'
import { saveEditorTabUntilStable } from '@/lib/documentSaveCoordinator'
import { readableError } from '@/lib/errors'
import type { DocumentDescriptor, EditorTab, UnsavedComparison } from '@/types'

import {
  activeTab,
  externalComparison,
  externalComparisons,
  sessions,
  setError,
  tabKey,
  updateWorktreeStatus,
  type WorkspaceSession,
} from './state'

const saveTimers = new Map<string, number>()
const saveTasks = new Map<string, Promise<void>>()

export function updateActiveContent(content: string) {
  const tab = activeTab.value
  if (!tab || tab.readOnly || tab.content === content) return
  tab.content = content
  tab.revision += 1
  tab.dirty = tab.content !== tab.diskContent
  tab.saveError = undefined
  scheduleSave(tab)
}

export function scheduleSave(tab: EditorTab) {
  const key = tabKey(tab.root, tab.path)
  const timer = saveTimers.get(key)
  if (timer) window.clearTimeout(timer)
  saveTimers.set(
    key,
    window.setTimeout(() => {
      saveTimers.delete(key)
      void saveTab(tab).catch(() => undefined)
    }, 1000),
  )
}

export async function saveTab(tab: EditorTab, expectedShaOverride?: string | null) {
  if (tab.readOnly || !isTauri()) return
  const key = tabKey(tab.root, tab.path)
  const timer = saveTimers.get(key)
  if (timer) {
    window.clearTimeout(timer)
    saveTimers.delete(key)
  }
  const existing = saveTasks.get(key)
  if (existing) return existing

  const task = (async () => {
    let expectedSha = expectedShaOverride
    do {
      await saveEditorTabUntilStable(
        tab,
        (current, content, currentExpectedSha, expectedMissing) =>
          nativeApi.saveDocument({
            request: {
              root: current.root,
              path: current.path,
              content,
              expectedSha256: currentExpectedSha ?? null,
              expectedMissing,
              encoding: current.encoding,
            },
          }),
        expectedSha,
        (reason) => handleSaveFailure(tab, reason),
      )
      expectedSha = undefined
      try {
        await refreshStatusForRoot(tab.root)
      } catch (reason) {
        setError(reason)
      }
    } while (tab.dirty)
  })()
    .finally(() => {
      saveTasks.delete(key)
      tab.saving = false
    })
  saveTasks.set(key, task)
  return task
}

export async function handleSaveFailure(tab: EditorTab, reason: unknown) {
  tab.saveError = readableError(reason)
  if (isExternalChangeError(reason)) {
    let disk: Awaited<ReturnType<typeof nativeApi.readDocument>> | undefined
    try {
      disk = await nativeApi.readDocument({ root: tab.root, path: tab.path })
    } catch (reason) {
      if (!hasNativeErrorCode(reason, 'fileNotFound')) {
        setError(reason)
        return
      }
    }
    enqueueExternalComparison({
      tabKey: tabKey(tab.root, tab.path),
      root: tab.root,
      path: tab.path,
      disk: disk?.content ?? '',
      editor: tab.content,
      editorRevision: tab.revision,
      diskMissing: !disk,
      externalChange: true,
    })
  } else {
    setError(reason)
  }
}

export async function chooseExternalVersion(choice: 'disk' | 'editor') {
  const comparison = externalComparison.value
  const session = comparison ? sessions.get(comparison.root) : undefined
  const tab = session?.tabs.find(
    (candidate) => tabKey(candidate.root, candidate.path) === comparison?.tabKey,
  )
  if (!comparison || !session || !tab) return
  try {
    if (choice === 'disk') {
      if (comparison.diskMissing) {
        removeTab(session, tab)
        removeExternalComparison(comparison.tabKey)
        return
      }
      const disk = await nativeApi.readDocument({ root: tab.root, path: tab.path })
      tab.content = disk.content
      tab.diskContent = disk.content
      tab.sha256 = disk.sha256
      tab.modifiedMs = disk.modifiedMs
      tab.revision += 1
      tab.savedRevision = tab.revision
      tab.dirty = false
      tab.saveError = undefined
    } else {
      const disk = comparison.diskMissing
        ? undefined
        : await nativeApi.readDocument({ root: tab.root, path: tab.path })
      if (disk) tab.sha256 = disk.sha256
      tab.dirty = true
      await saveTab(tab, disk?.sha256 ?? null)
    }
    removeExternalComparison(comparison.tabKey)
  } catch (reason) {
    setError(reason)
  }
}

export async function closeTab(tab: EditorTab) {
  const session = sessions.get(tab.root)
  if (!session || !session.tabs.includes(tab)) return
  try {
    await saveTab(tab)
    removeTab(session, tab)
  } catch {
    // The tab remains open with the visible save error and recovery choices.
  }
}

export async function refreshStatusForRoot(root: string) {
  if (!root || !isTauri()) return
  updateWorktreeStatus(root, await nativeApi.repositoryStatus({ root }))
}

export async function flushAll(root?: string) {
  if (!isTauri()) return
  const targetSessions = root
    ? [sessions.get(root)].filter((session): session is WorkspaceSession => Boolean(session))
    : [...sessions.values()]
  for (const session of targetSessions) {
    for (const tab of [...session.tabs]) {
      if (tab.dirty || saveTasks.has(tabKey(tab.root, tab.path))) {
        await saveTab(tab)
      }
    }
  }
}

export async function reconcileOpenTabs(
  session: WorkspaceSession,
  nextDocuments: DocumentDescriptor[],
  generation: number,
) {
  const available = new Map(nextDocuments.map((document) => [document.path, document]))
  for (const tab of [...session.tabs]) {
    if (session.loadGeneration !== generation) return
    const descriptor = available.get(tab.path)
    if (!descriptor || !['markdown', 'text'].includes(descriptor.kind)) {
      if (tab.dirty) {
        enqueueExternalComparison({
          tabKey: tabKey(tab.root, tab.path),
          root: tab.root,
          path: tab.path,
          disk: '',
          editor: tab.content,
          editorRevision: tab.revision,
          diskMissing: true,
          externalChange: true,
        })
      } else {
        removeTab(session, tab)
      }
      continue
    }
    try {
      const disk = await nativeApi.readDocument({ root: tab.root, path: tab.path })
      if (session.loadGeneration !== generation) return
      if (disk.sha256 === tab.sha256) continue
      if (tab.dirty) {
        enqueueExternalComparison({
          tabKey: tabKey(tab.root, tab.path),
          root: tab.root,
          path: tab.path,
          disk: disk.content,
          editor: tab.content,
          editorRevision: tab.revision,
          diskMissing: false,
          externalChange: true,
        })
      } else {
        tab.content = disk.content
        tab.diskContent = disk.content
        tab.sha256 = disk.sha256
        tab.modifiedMs = disk.modifiedMs
        tab.encoding = disk.encoding
        tab.lineEnding = disk.lineEnding
        tab.revision += 1
        tab.savedRevision = tab.revision
      }
    } catch (reason) {
      setError(reason)
    }
  }
}

export function enqueueExternalComparison(comparison: UnsavedComparison) {
  const existing = externalComparisons.value.findIndex(
    (candidate) => candidate.tabKey === comparison.tabKey,
  )
  if (existing >= 0) externalComparisons.value[existing] = comparison
  else externalComparisons.value.push(comparison)
}

export function removeExternalComparison(key: string) {
  externalComparisons.value = externalComparisons.value.filter(
    (comparison) => comparison.tabKey !== key,
  )
}

export function removeTab(session: WorkspaceSession, tab: EditorTab) {
  const index = session.tabs.indexOf(tab)
  if (index < 0) return
  const key = tabKey(tab.root, tab.path)
  const timer = saveTimers.get(key)
  if (timer) {
    window.clearTimeout(timer)
    saveTimers.delete(key)
  }
  removeExternalComparison(key)
  session.tabs.splice(index, 1)
  if (session.activeTabKey === key) {
    const next = session.tabs[Math.min(index, session.tabs.length - 1)]
    session.activeTabKey = next ? tabKey(next.root, next.path) : undefined
  }
}

export function disposeSession(root: string) {
  const session = sessions.get(root)
  for (const tab of session?.tabs ?? []) {
    const key = tabKey(tab.root, tab.path)
    const timer = saveTimers.get(key)
    if (timer) window.clearTimeout(timer)
    saveTimers.delete(key)
    saveTasks.delete(key)
    removeExternalComparison(key)
  }
  sessions.delete(root)
}

function isExternalChangeError(reason: unknown) {
  return hasNativeErrorCode(reason, 'externalChange')
}

function hasNativeErrorCode(reason: unknown, code: string) {
  return Boolean(
    reason &&
      typeof reason === 'object' &&
      'code' in reason &&
      reason.code === code,
  )
}
