import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { type as osType } from '@tauri-apps/plugin-os'
import { onBeforeUnmount, onMounted, ref, watch } from 'vue'

import type { WorkspaceApi } from '@/composables/useWorkspace'
import { i18n } from '@/i18n'
import { isTauri, nativeApi } from '@/lib/api'
import { windowService } from '@/lib/windowService'
import type {
  WorkspaceChangedEvent,
  WorkspaceForgottenEvent,
  WorkspaceWatchErrorEvent,
} from '@/types'

type Workspace = Pick<
  WorkspaceApi,
  | 'activeRoot'
  | 'activeWorkspace'
  | 'flushAll'
  | 'handleWorkspaceChanged'
  | 'handleWorkspaceForgotten'
  | 'initializeWorkspace'
  | 'refreshActive'
  | 'reportError'
  | 'search'
  | 'searchQuery'
>

export function useWorkspaceLifecycle(
  workspace: Workspace,
) {
  const viewportMobile = ref(window.innerWidth < 760)
  const nativeAndroid = ref(false)
  const ready = ref(false)
  const platformPulse = ref(0)
  const filesystemRefreshTimers = new Map<string, number>()
  const pendingFilesystemRefreshes = new Set<string>()
  const refreshingRoots = new Set<string>()
  const changedPathsByRoot = new Map<string, Set<string>>()
  let refreshTimer: number | undefined
  let searchTimer: number | undefined
  let unlistenWorkspaceChanges: UnlistenFn | undefined
  let unlistenWorkspaceWatchError: UnlistenFn | undefined
  let unlistenWorkspaceForgotten: UnlistenFn | undefined
  let unlistenCloseRequested: UnlistenFn | undefined
  let closingWindow = false
  let watchedRoot: string | undefined

  function clearWorkspaceTimers(roots: Iterable<string>) {
    for (const root of roots) {
      const timer = filesystemRefreshTimers.get(root)
      if (timer) window.clearTimeout(timer)
      filesystemRefreshTimers.delete(root)
      pendingFilesystemRefreshes.delete(root)
      changedPathsByRoot.delete(root)
    }
  }

  function queueWorkspaceRefresh(root: string, paths: string[] = []) {
    pendingFilesystemRefreshes.add(root)
    let changedPaths = changedPathsByRoot.get(root)
    if (!changedPaths) {
      changedPaths = new Set()
      changedPathsByRoot.set(root, changedPaths)
    }
    for (const path of paths) changedPaths.add(path)
    const timer = filesystemRefreshTimers.get(root)
    if (timer) window.clearTimeout(timer)
    filesystemRefreshTimers.set(
      root,
      window.setTimeout(() => {
        filesystemRefreshTimers.delete(root)
        void drainWorkspaceRefresh(root)
      }, 180),
    )
  }

  async function drainWorkspaceRefresh(root: string) {
    if (refreshingRoots.has(root)) return
    refreshingRoots.add(root)
    try {
      while (pendingFilesystemRefreshes.delete(root)) {
        const paths = [...(changedPathsByRoot.get(root) ?? [])]
        changedPathsByRoot.delete(root)
        await workspace.handleWorkspaceChanged(root, paths)
      }
    } finally {
      refreshingRoots.delete(root)
      if (pendingFilesystemRefreshes.has(root)) queueWorkspaceRefresh(root)
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
        workspace.reportError(reason)
      })
    }
  }

  function handleWindowFocus() {
    if (isTauri()) platformPulse.value += 1
  }

  onMounted(async () => {
    window.addEventListener('resize', updateViewport)
    window.addEventListener('focus', handleWindowFocus)
    document.addEventListener('visibilitychange', handleVisibilityChange)
    if (isTauri()) {
      try {
        nativeAndroid.value = osType() === 'android'
        unlistenWorkspaceChanges = await listen<WorkspaceChangedEvent>(
          'workspace-changed',
          (event) => {
            const { root } = event.payload
            queueWorkspaceRefresh(root, event.payload.paths)
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
            workspace.reportError(i18n.global.t('app.workspaceWatchFailed', {
              root: event.payload.root,
              message: event.payload.message,
            }))
          },
        )
        unlistenCloseRequested = await windowService.onCloseRequested(
          async (event) => {
            event.preventDefault()
            if (closingWindow) return
            closingWindow = true
            try {
              await workspace.flushAll()
              if (watchedRoot) {
                await nativeApi.unwatchWorkspace({ root: watchedRoot })
                watchedRoot = undefined
              }
              await windowService.destroyAfterFlush()
            } catch (reason) {
              closingWindow = false
              workspace.reportError(reason)
            }
          },
        )
      } catch (reason) {
        workspace.reportError(reason)
      }
    }
    await workspace.initializeWorkspace()
    ready.value = true
    platformPulse.value += 1
    refreshTimer = window.setInterval(() => {
      if (document.visibilityState === 'visible') void refreshForPlatform()
    }, 5 * 60 * 1000)
  })

  onBeforeUnmount(() => {
    window.removeEventListener('resize', updateViewport)
    window.removeEventListener('focus', handleWindowFocus)
    document.removeEventListener('visibilitychange', handleVisibilityChange)
    if (refreshTimer) window.clearInterval(refreshTimer)
    if (searchTimer) window.clearTimeout(searchTimer)
    clearWorkspaceTimers(filesystemRefreshTimers.keys())
    if (watchedRoot && isTauri()) {
      void nativeApi.unwatchWorkspace({ root: watchedRoot }).catch(() => undefined)
    }
    unlistenWorkspaceChanges?.()
    unlistenWorkspaceWatchError?.()
    unlistenWorkspaceForgotten?.()
    unlistenCloseRequested?.()
  })

  watch(workspace.activeRoot, (nextRoot, previousRoot) => {
    if (previousRoot && isTauri()) {
      clearWorkspaceTimers([previousRoot])
      void nativeApi.unwatchWorkspace({ root: previousRoot }).catch(() => undefined)
    }
    watchedRoot = nextRoot
    if (nextRoot && isTauri()) {
      void nativeApi
        .watchWorkspace({ root: nextRoot })
        .catch((reason) => {
          workspace.reportError(reason)
        })
    }
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
    ready,
    platformPulse,
    clearWorkspaceTimers,
    refreshForPlatform,
  }
}
