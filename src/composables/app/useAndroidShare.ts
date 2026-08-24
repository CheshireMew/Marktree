import { nextTick, ref, type Ref } from 'vue'
import { useI18n } from 'vue-i18n'

import type { WorkspaceApi } from '@/composables/useWorkspace'
import { isTauri, nativeApi } from '@/lib/api'
import type { PendingAndroidShare } from '@/types'

type Workspace = Pick<
  WorkspaceApi,
  | 'activeRoot'
  | 'activeWorkspace'
  | 'addWorkspace'
  | 'loadDocuments'
  | 'notify'
  | 'openDocument'
  | 'reportError'
  | 'workspaces'
>

export function useAndroidShare(
  workspace: Workspace,
  nativeAndroid: Ref<boolean>,
  insertIntoEditor: (markdown: string) => void,
) {
  const { t } = useI18n()
  const pending = ref<PendingAndroidShare>()
  const selectedRoot = ref('')
  const selectedDirectory = ref('')
  const directories = ref<string[]>([])
  const importing = ref(false)
  let detecting = false

  async function detect() {
    if (detecting || pending.value || !nativeAndroid.value || !isTauri()) return
    detecting = true
    try {
      const share = await nativeApi.takePendingAndroidShare()
      if (!share) return
      pending.value = share
      const root = workspace.activeWorkspace.value?.root ?? workspace.workspaces.value[0]?.root ?? ''
      await selectRoot(root)
    } catch (reason) {
      workspace.reportError(reason)
    } finally {
      detecting = false
    }
  }

  async function selectRoot(root: string) {
    selectedRoot.value = root
    selectedDirectory.value = ''
    if (!root) {
      directories.value = []
      return
    }
    try {
      directories.value = await nativeApi.listWorkspaceDirectories({ root })
    } catch (reason) {
      workspace.reportError(reason)
    }
  }

  async function importShare(documentPath?: string) {
    const share = pending.value
    if (!share || importing.value) return
    importing.value = true
    try {
      const result = await nativeApi.importAndroidShare({
        request: {
          share,
          root: selectedRoot.value || null,
          targetDirectory: selectedDirectory.value,
          documentPath: documentPath ?? null,
        },
      })
      const sameWorkspace = workspace.activeWorkspace.value?.id === result.workspace.id
      if (sameWorkspace) {
        await workspace.loadDocuments(workspace.activeRoot.value)
      } else {
        await workspace.addWorkspace(result.workspace)
      }
      if (result.openPath) await workspace.openDocument(result.openPath)
      if (result.insertMarkdown) {
        await nextTick()
        insertIntoEditor(result.insertMarkdown)
      }
      workspace.notify(result.archiveImported
        ? t('app.workspaceArchiveImported')
        : t('app.shareImported'))
      pending.value = undefined
    } catch (reason) {
      workspace.reportError(reason)
    } finally {
      importing.value = false
    }
  }

  async function exportWorkspace() {
    const root = workspace.activeRoot.value
    if (!root) return
    try {
      const result = await nativeApi.exportAndroidWorkspaceArchive({ root })
      workspace.notify(t('app.workspaceArchiveReady', {
        count: result.fileCount,
      }))
    } catch (reason) {
      workspace.reportError(reason)
    }
  }

  return {
    pending,
    selectedRoot,
    selectedDirectory,
    directories,
    importing,
    detect,
    selectRoot,
    importShare,
    exportWorkspace,
  }
}
