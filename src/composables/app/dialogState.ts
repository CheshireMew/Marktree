import { reactive, ref } from 'vue'

export type AppModal =
  | 'clone'
  | 'document'
  | 'worktree'
  | 'settings'
  | 'mobileWorkspace'

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
}

export function useDialogState() {
  const modal = ref<AppModal>()
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
  })
  return { modal, form }
}
