import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { type as osType } from '@tauri-apps/plugin-os'
import { onBeforeUnmount, onMounted, ref, watch } from 'vue'

import type { useWorkspace } from '@/composables/useWorkspace'
import { i18n } from '@/i18n'
import { isTauri, nativeApi } from '@/lib/api'
import { readableError } from '@/lib/errors'
import type {
  WorkspaceChangedEvent,
  WorkspaceForgottenEvent,
  WorkspaceWatchErrorEvent,
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
  let unlistenWorkspaceChanges: UnlistenFn | undefined
  let unlistenWorkspaceWatchError: UnlistenFn | undefined
  let unlistenWorkspaceForgotten: UnlistenFn | undefined
  let unlistenCloseRequested: UnlistenFn | undefined
  let closingWindow = false

  function clearWorkspaceTimers(roots: Iterable<string>) {
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
        Boolean(workspace.activeWorkspace.value?.git?.remoteUrl),
        false,
      )
    } catch {
      // Offline Git workspaces remain editable and retain the last fetched status.
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
        unlistenWorkspaceChanges = await listen<WorkspaceChangedEvent>(
          'workspace-changed',
          (event) => {
            const { root } = event.payload
            clearWorkspaceTimers([root])
            filesystemRefreshTimers.set(
              root,
              window.setTimeout(() => {
                filesystemRefreshTimers.delete(root)
                void workspace.handleWorkspaceChanged(root)
              }, 180),
            )
          },
        )
        unlistenWorkspaceForgotten = await listen<WorkspaceForgottenEvent>(
          'workspace-forgotten',
          (event) => {
            clearWorkspaceTimers(event.payload.roots)
            void workspace.handleWorkspaceForgotten(
              event.payload.workspaceId,
              event.payload.roots,
            )
          },
        )
        unlistenWorkspaceWatchError = await listen<WorkspaceWatchErrorEvent>(
          'workspace-watch-error',
          (event) => {
            workspace.error.value = i18n.global.t('app.workspaceWatchFailed', {
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
    clearWorkspaceTimers(filesystemRefreshTimers.keys())
    unlistenWorkspaceChanges?.()
    unlistenWorkspaceWatchError?.()
    unlistenWorkspaceForgotten?.()
    unlistenCloseRequested?.()
  })

  watch(workspace.activeRoot, () => {
    onActiveRootChanged()
    if (workspace.activeRoot.value && isTauri()) {
      void nativeApi
        .watchWorkspace({ root: workspace.activeRoot.value })
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
    clearWorkspaceTimers,
    refreshForPlatform,
  }
}
