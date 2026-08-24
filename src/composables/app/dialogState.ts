import { reactive, ref } from 'vue'

import { readableError } from '@/lib/errors'

export type AppModal =
  | 'clone'
  | 'document'
  | 'worktree'
  | 'settings'
  | 'mobileWorkspace'
  | 'workspaceEntry'
  | 'desktopWorkspace'
  | 'confirmation'

export type WorkspaceEntryAction = 'newFolder' | 'rename' | 'duplicate' | 'trash'

export interface ConfirmationDialogState {
  title: string
  message: string
  confirmLabel: string
  danger?: boolean
}

export interface WorkspaceDialogForm {
  remoteUrl: string
  destination: string
  documentPath: string
  worktreeName: string
  worktreePath: string
  worktreeBranch: string
  worktreeStart: string
  credentialUsername: string
  credentialToken: string
  assetsDir: string
  ignoreRules: string
  workspaceName: string
  entryAction: WorkspaceEntryAction
  entryDirectory: string
  entrySourcePath: string
  entryName: string
  entryOriginalName: string
}

export function useDialogState() {
  const modal = ref<AppModal>()
  const dialogBusy = ref(false)
  const dialogError = ref('')
  const form = reactive<WorkspaceDialogForm>({
    remoteUrl: '',
    destination: '',
    documentPath: '',
    worktreeName: '',
    worktreePath: '',
    worktreeBranch: '',
    worktreeStart: 'HEAD',
    credentialUsername: '',
    credentialToken: '',
    assetsDir: 'assets',
    ignoreRules: '',
    workspaceName: '',
    entryAction: 'newFolder',
    entryDirectory: '',
    entrySourcePath: '',
    entryName: '',
    entryOriginalName: '',
  })

  function openDialog(value: AppModal) {
    dialogError.value = ''
    dialogBusy.value = false
    modal.value = value
  }

  function closeDialog() {
    if (dialogBusy.value) return
    dialogError.value = ''
    modal.value = undefined
  }

  async function runDialogAction<T>(action: () => Promise<T>): Promise<T | undefined> {
    if (dialogBusy.value) return undefined
    dialogBusy.value = true
    dialogError.value = ''
    try {
      return await action()
    } catch (reason) {
      dialogError.value = readableError(reason)
      return undefined
    } finally {
      dialogBusy.value = false
    }
  }

  return {
    modal,
    form,
    dialogBusy,
    dialogError,
    openDialog,
    closeDialog,
    runDialogAction,
  }
}
