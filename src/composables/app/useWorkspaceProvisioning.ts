import { WebviewWindow } from '@tauri-apps/api/webviewWindow'
import { open } from '@tauri-apps/plugin-dialog'
import { watch, type Ref } from 'vue'

import type { useWorkspace } from '@/composables/useWorkspace'
import { i18n } from '@/i18n'
import { isTauri, nativeApi } from '@/lib/api'
import { readableError } from '@/lib/errors'
import type { WorktreeDescriptor } from '@/types'

import type { AppModal, WorkspaceDialogForm } from './dialogState'

type Workspace = ReturnType<typeof useWorkspace>

export function useWorkspaceProvisioning(
  workspace: Workspace,
  modal: Ref<AppModal | undefined>,
  form: WorkspaceDialogForm,
  nativeAndroid: Ref<boolean>,
  addMenuOpen: Ref<boolean>,
  cloneCredentialId: Ref<string | undefined>,
  prepareCloneCredentials: () => Promise<void>,
  closeModal: () => void,
) {
  async function chooseWorkspace(action: 'open' | 'create') {
    addMenuOpen.value = false
    if (!isTauri()) return
    if (nativeAndroid.value && action === 'create') {
      form.workspaceName = ''
      modal.value = 'mobileWorkspace'
      return
    }
    try {
      const selected = await open({ directory: true, multiple: false })
      if (!selected) return
      let path = selected
      if (action === 'create') {
        const name = window.prompt(i18n.global.t('app.workspaceName'))
        if (!name?.trim()) return
        const separator = selected.includes('\\') ? '\\' : '/'
        path = `${selected}${separator}${name.trim()}`
      }
      const descriptor =
        action === 'open'
          ? await nativeApi.openWorkspace({ path })
          : await nativeApi.createWorkspace({ path })
      await workspace.addWorkspace(descriptor)
    } catch (reason) {
      workspace.error.value = readableError(reason)
    }
  }

  async function chooseCloneDestination() {
    try {
      const selected = await open({ directory: true, multiple: false })
      if (selected) form.destination = selected
    } catch (reason) {
      workspace.error.value = readableError(reason)
    }
  }

  async function openCloneDialog() {
    addMenuOpen.value = false
    modal.value = 'clone'
    form.remoteUrl = ''
    form.workspaceName = ''
    await prepareCloneCredentials()
  }

  async function cloneGitWorkspace() {
    if (!form.remoteUrl.trim() || (!nativeAndroid.value && !form.destination.trim())) {
      return
    }
    try {
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
      closeModal()
    } catch (reason) {
      workspace.error.value = readableError(reason)
    }
  }

  async function createMobileWorkspace() {
    if (!form.workspaceName.trim()) return
    try {
      const descriptor = await nativeApi.createMobileWorkspace({
        workspaceName: form.workspaceName.trim(),
      })
      await workspace.addWorkspace(descriptor)
      closeModal()
    } catch (reason) {
      workspace.error.value = readableError(reason)
    }
  }

  function openNewDocument(directory = '') {
    form.documentPath = directory ? `${directory}/` : ''
    modal.value = 'document'
  }

  async function createDocument() {
    if (!form.documentPath.trim()) return
    if (await workspace.createDocument(form.documentPath.trim())) closeModal()
  }

  function openWorktreeDialog() {
    const activeWorkspace = workspace.activeWorkspace.value
    if (!activeWorkspace?.git) return
    form.worktreeName = ''
    form.worktreeBranch = ''
    form.worktreeStart = workspace.activeWorktree.value?.branch ?? 'HEAD'
    form.worktreePath = ''
    modal.value = 'worktree'
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
    try {
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
      closeModal()
    } catch (reason) {
      workspace.error.value = readableError(reason)
    }
  }

  function openWorktreeWindow(worktree: WorktreeDescriptor) {
    if (!isTauri()) return
    try {
      const label = `workspace-${crypto.randomUUID()}`
      new WebviewWindow(label, {
        url: `/?root=${encodeURIComponent(workspace.activeWorkspace.value?.root ?? worktree.path)}&worktree=${encodeURIComponent(worktree.path)}`,
        title: `${workspace.activeWorkspace.value?.name ?? 'Marktree'} · ${worktree.name}`,
        width: 1280,
        height: 820,
        minWidth: 900,
        minHeight: 600,
        decorations: false,
      })
    } catch (reason) {
      workspace.error.value = readableError(reason)
    }
  }

  return {
    chooseWorkspace,
    chooseCloneDestination,
    openCloneDialog,
    cloneGitWorkspace,
    createMobileWorkspace,
    openNewDocument,
    createDocument,
    openWorktreeDialog,
    createWorktree,
    openWorktreeWindow,
  }
}
