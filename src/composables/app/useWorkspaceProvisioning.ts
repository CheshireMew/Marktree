import { open } from '@tauri-apps/plugin-dialog'
import { watch, type Ref } from 'vue'

import type { WorkspaceApi } from '@/composables/useWorkspace'
import { i18n } from '@/i18n'
import { isTauri, nativeApi } from '@/lib/api'
import { windowService } from '@/lib/windowService'
import type { WorktreeDescriptor } from '@/types'

import type { AppModal, WorkspaceDialogForm } from './dialogState'

type Workspace = Pick<
  WorkspaceApi,
  | 'activeWorkspace'
  | 'activeWorktree'
  | 'addWorkspace'
  | 'createDocument'
  | 'error'
  | 'refreshActive'
  | 'reportError'
>

export function useWorkspaceProvisioning(
  workspace: Workspace,
  form: WorkspaceDialogForm,
  nativeAndroid: Ref<boolean>,
  addMenuOpen: Ref<boolean>,
  cloneCredentialId: Ref<string | undefined>,
  prepareCloneCredentials: () => Promise<void>,
  openDialog: (value: AppModal) => void,
  runDialogAction: <T>(action: () => Promise<T>) => Promise<T | undefined>,
  closeModal: () => void,
) {
  async function chooseWorkspace(action: 'open' | 'create') {
    addMenuOpen.value = false
    if (!isTauri()) return
    if (nativeAndroid.value && action === 'create') {
      form.workspaceName = ''
      openDialog('mobileWorkspace')
      return
    }
    try {
      const selected = await open({ directory: true, multiple: false })
      if (!selected) return
      if (action === 'create') {
        form.destination = selected
        form.workspaceName = ''
        openDialog('desktopWorkspace')
        return
      }
      const descriptor = await nativeApi.openWorkspace({ path: selected })
      await workspace.addWorkspace(descriptor)
    } catch (reason) {
      workspace.reportError(reason)
    }
  }

  async function chooseCloneDestination() {
    await runDialogAction(async () => {
      const selected = await open({ directory: true, multiple: false })
      if (selected) form.destination = selected
      return true
    })
  }

  async function openCloneDialog() {
    addMenuOpen.value = false
    openDialog('clone')
    form.remoteUrl = ''
    form.workspaceName = ''
    await prepareCloneCredentials()
  }

  async function cloneGitWorkspace() {
    if (!form.remoteUrl.trim() || (!nativeAndroid.value && !form.destination.trim())) {
      return
    }
    const created = await runDialogAction(async () => {
      if (form.credentialToken.trim()) {
        cloneCredentialId.value ??= `clone-${crypto.randomUUID()}`
        await nativeApi.saveCredential({
          input: {
            id: cloneCredentialId.value,
            username: form.credentialUsername.trim(),
            token: form.credentialToken.trim(),
          },
        })
      }
      const inferredName =
        form.workspaceName.trim() ||
        form.remoteUrl.split('/').at(-1)?.replace(/\.git$/i, '') ||
        'workspace'
      const descriptor = nativeAndroid.value
        ? await nativeApi.cloneMobileGitWorkspace({
            remoteUrl: form.remoteUrl.trim(),
            workspaceName: inferredName,
            credentialId: cloneCredentialId.value ?? null,
          })
        : await nativeApi.cloneGitWorkspace({
            remoteUrl: form.remoteUrl.trim(),
            path: form.destination.trim(),
            credentialId: cloneCredentialId.value ?? null,
          })
      await workspace.addWorkspace(descriptor)
      return true
    })
    if (created) closeModal()
  }

  async function createMobileWorkspace() {
    if (!form.workspaceName.trim()) return
    const created = await runDialogAction(async () => {
      const descriptor = await nativeApi.createMobileWorkspace({
        workspaceName: form.workspaceName.trim(),
      })
      await workspace.addWorkspace(descriptor)
      return true
    })
    if (created) closeModal()
  }

  async function createDesktopWorkspace() {
    if (!form.destination.trim() || !form.workspaceName.trim()) return
    const created = await runDialogAction(async () => {
      const separator = form.destination.includes('\\') ? '\\' : '/'
      const path = `${form.destination.replace(/[\\/]$/, '')}${separator}${form.workspaceName.trim()}`
      const descriptor = await nativeApi.createWorkspace({ path })
      await workspace.addWorkspace(descriptor)
      return true
    })
    if (created) closeModal()
  }

  function openNewDocument(directory = '') {
    form.documentPath = directory ? `${directory}/` : ''
    openDialog('document')
  }

  async function createDocument() {
    if (!form.documentPath.trim()) return
    const created = await runDialogAction(async () => {
      if (!await workspace.createDocument(form.documentPath.trim())) {
        throw new Error(workspace.error.value || i18n.global.t('app.newDocument'))
      }
      return true
    })
    if (created) closeModal()
  }

  function openWorktreeDialog() {
    const activeWorkspace = workspace.activeWorkspace.value
    if (!activeWorkspace?.git) return
    form.worktreeName = ''
    form.worktreeBranch = ''
    form.worktreeStart = workspace.activeWorktree.value?.branch ?? 'HEAD'
    form.worktreePath = ''
    openDialog('worktree')
  }

  watch(
    () => form.worktreeName,
    (name) => {
      const activeWorkspace = workspace.activeWorkspace.value
      if (!activeWorkspace?.git || !name.trim()) return
      const separator = activeWorkspace.root.includes('\\') ? '\\' : '/'
      const parent = activeWorkspace.root.replace(/[\\/][^\\/]+$/, '')
      form.worktreePath = `${parent}${separator}${activeWorkspace.name}-${name.trim()}`
      if (!form.worktreeBranch) form.worktreeBranch = name.trim()
    },
  )

  async function createWorktree() {
    const activeWorkspace = workspace.activeWorkspace.value
    if (!activeWorkspace?.git) return
    const created = await runDialogAction(async () => {
      await nativeApi.createWorktree({
        request: {
          root: activeWorkspace.root,
          name: form.worktreeName.trim(),
          path: form.worktreePath.trim(),
          branch: form.worktreeBranch.trim(),
          startPoint: form.worktreeStart.trim() || null,
        },
      })
      await workspace.refreshActive()
      return true
    })
    if (created) closeModal()
  }

  async function openWorktreeWindow(worktree: WorktreeDescriptor) {
    if (!isTauri()) return
    try {
      await windowService.openWorkspaceWindow({
        root: workspace.activeWorkspace.value?.root ?? worktree.path,
        worktree: worktree.path,
        title: `${workspace.activeWorkspace.value?.name ?? 'Marktree'} · ${worktree.name}`,
      })
    } catch (reason) {
      workspace.reportError(reason)
    }
  }

  return {
    chooseWorkspace,
    chooseCloneDestination,
    openCloneDialog,
    cloneGitWorkspace,
    createMobileWorkspace,
    createDesktopWorkspace,
    openNewDocument,
    createDocument,
    openWorktreeDialog,
    createWorktree,
    openWorktreeWindow,
  }
}
