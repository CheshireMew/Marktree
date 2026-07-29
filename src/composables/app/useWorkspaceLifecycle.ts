import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { type as osType } from '@tauri-apps/plugin-os'
import { onBeforeUnmount, onMounted, ref, watch } from 'vue'

import type { useWorkspace } from '@/composables/useWorkspace'
import { i18n } from '@/i18n'
import { isTauri, nativeApi } from '@/lib/api'
import { readableError } from '@/lib/errors'
import type {
  RepositoryChangedEvent,
  RepositoryForgottenEvent,
  RepositoryWatchErrorEvent,
} from '@/types'

type Workspace = ReturnType<typeof useWorkspace>

export function useWorkspaceLifecycle(
  workspace: Workspace,
  onActiveRootChanged: () => void,
) {
  const viewportMobile = ref(window.innerWidth < 760)
  const nativeAndroid = ref(false)
  const filesystemRefreshTimers = new Map<string, number>()
  let refreshTimer: number | undefined
  let searchTimer: number | undefined
  let unlistenRepositoryChanges: UnlistenFn | undefined
  let unlistenRepositoryWatchError: UnlistenFn | undefined
  let unlistenRepositoryForgotten: UnlistenFn | undefined
  let unlistenCloseRequested: UnlistenFn | undefined
  let closingWindow = false

  function clearRepositoryTimers(roots: Iterable<string>) {
    for (const root of roots) {
      const timer = filesystemRefreshTimers.get(root)
      if (timer) window.clearTimeout(timer)
      filesystemRefreshTimers.delete(root)
    }
  }

  async function refreshForPlatform() {
    if (!workspace.activeRoot.value || !isTauri()) return
    try {
      await workspace.refreshActive(
        Boolean(workspace.activeRepository.value?.remoteUrl),
        false,
      )
    } catch {
      // Offline repositories remain editable and retain the last fetched status.
    }
  }

  function handleVisibilityChange() {
    if (document.visibilityState === 'hidden') {
      void workspace.flushAll().catch((reason) => {
        workspace.error.value = readableError(reason)
      })
    }
  }

  onMounted(async () => {
    window.addEventListener('resize', updateViewport)
    document.addEventListener('visibilitychange', handleVisibilityChange)
    if (isTauri()) {
      try {
        nativeAndroid.value = osType() === 'android'
        unlistenRepositoryChanges = await listen<RepositoryChangedEvent>(
          'repository-changed',
          (event) => {
            const { root } = event.payload
            clearRepositoryTimers([root])
            filesystemRefreshTimers.set(
              root,
              window.setTimeout(() => {
                filesystemRefreshTimers.delete(root)
                void workspace.handleRepositoryChanged(root)
              }, 180),
            )
          },
        )
        unlistenRepositoryForgotten = await listen<RepositoryForgottenEvent>(
          'repository-forgotten',
          (event) => {
            clearRepositoryTimers(event.payload.worktreeRoots)
            void workspace.handleRepositoryForgotten(
              event.payload.repositoryId,
              event.payload.worktreeRoots,
            )
          },
        )
        unlistenRepositoryWatchError = await listen<RepositoryWatchErrorEvent>(
          'repository-watch-error',
          (event) => {
            workspace.error.value = i18n.global.t('app.repositoryWatchFailed', {
              root: event.payload.root,
              message: event.payload.message,
            })
          },
        )
        unlistenCloseRequested = await getCurrentWindow().onCloseRequested(
          async (event) => {
            if (closingWindow) return
            event.preventDefault()
            try {
              await workspace.flushAll()
              closingWindow = true
              await getCurrentWindow().destroy()
            } catch (reason) {
              workspace.error.value = readableError(reason)
            }
          },
        )
      } catch (reason) {
        workspace.error.value = readableError(reason)
      }
    }
    await workspace.initializeWorkspace()
    await refreshForPlatform()
    refreshTimer = window.setInterval(() => {
      if (document.visibilityState === 'visible') void refreshForPlatform()
    }, 5 * 60 * 1000)
  })

  onBeforeUnmount(() => {
    window.removeEventListener('resize', updateViewport)
    document.removeEventListener('visibilitychange', handleVisibilityChange)
    if (refreshTimer) window.clearInterval(refreshTimer)
    if (searchTimer) window.clearTimeout(searchTimer)
    clearRepositoryTimers(filesystemRefreshTimers.keys())
    unlistenRepositoryChanges?.()
    unlistenRepositoryWatchError?.()
    unlistenRepositoryForgotten?.()
    unlistenCloseRequested?.()
  })

  watch(workspace.activeRoot, () => {
    onActiveRootChanged()
    if (workspace.activeRoot.value && isTauri()) {
      void nativeApi
        .watchRepository({ root: workspace.activeRoot.value })
        .catch((reason) => {
          workspace.error.value = readableError(reason)
        })
    }
    void refreshForPlatform()
  })

  watch(workspace.searchQuery, () => {
    if (searchTimer) window.clearTimeout(searchTimer)
    searchTimer = window.setTimeout(() => void workspace.search(), 240)
  })

  function updateViewport() {
    viewportMobile.value = window.innerWidth < 760
  }

  return {
    viewportMobile,
    nativeAndroid,
    clearRepositoryTimers,
    refreshForPlatform,
  }
}
