import { reactive, ref } from 'vue'

export type AppModal =
  | 'clone'
  | 'document'
  | 'worktree'
  | 'credentials'
  | 'mobileRepository'

export interface RepositoryDialogForm {
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
  repositoryName: string
}

export function useDialogState() {
  const modal = ref<AppModal>()
  const form = reactive<RepositoryDialogForm>({
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
    repositoryName: '',
  })
  return { modal, form }
}
