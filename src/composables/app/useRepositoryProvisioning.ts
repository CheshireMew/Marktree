import { WebviewWindow } from '@tauri-apps/api/webviewWindow'
import { open } from '@tauri-apps/plugin-dialog'
import { watch, type Ref } from 'vue'

import type { useWorkspace } from '@/composables/useWorkspace'
import { isTauri, nativeApi } from '@/lib/api'
import { readableError } from '@/lib/errors'
import type { WorktreeDescriptor } from '@/types'

import type { AppModal, RepositoryDialogForm } from './dialogState'

type Workspace = ReturnType<typeof useWorkspace>

export function useRepositoryProvisioning(
  workspace: Workspace,
  modal: Ref<AppModal | undefined>,
  form: RepositoryDialogForm,
  nativeAndroid: Ref<boolean>,
  addMenuOpen: Ref<boolean>,
  cloneCredentialId: Ref<string | undefined>,
  prepareCloneCredentials: () => Promise<void>,
  closeModal: () => void,
) {
  async function chooseRepository(action: 'open' | 'initialize') {
    addMenuOpen.value = false
    if (!isTauri()) return
    if (nativeAndroid.value && action === 'initialize') {
      form.repositoryName = ''
      modal.value = 'mobileRepository'
      return
    }
    try {
      const selected = await open({ directory: true, multiple: false })
      if (!selected) return
      const descriptor =
        action === 'open'
          ? await nativeApi.openRepository({ path: selected })
          : await nativeApi.initializeRepository({ path: selected })
      await workspace.addRepository(descriptor)
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
    form.repositoryName = ''
    await prepareCloneCredentials()
  }

  async function cloneRepository() {
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
        form.repositoryName.trim() ||
        form.remoteUrl.split('/').at(-1)?.replace(/\.git$/i, '') ||
        'repository'
      const descriptor = nativeAndroid.value
        ? await nativeApi.cloneMobileRepository({
            remoteUrl: form.remoteUrl.trim(),
            repositoryName: inferredName,
            credentialId: cloneCredentialId.value ?? null,
          })
        : await nativeApi.cloneRepository({
            remoteUrl: form.remoteUrl.trim(),
            path: form.destination.trim(),
            credentialId: cloneCredentialId.value ?? null,
          })
      await workspace.addRepository(descriptor)
      closeModal()
    } catch (reason) {
      workspace.error.value = readableError(reason)
    }
  }

  async function initializeMobileRepository() {
    if (!form.repositoryName.trim()) return
    try {
      const descriptor = await nativeApi.initializeMobileRepository({
        repositoryName: form.repositoryName.trim(),
      })
      await workspace.addRepository(descriptor)
      closeModal()
    } catch (reason) {
      workspace.error.value = readableError(reason)
    }
  }

  function openNewDocument() {
    form.documentPath = ''
    modal.value = 'document'
  }

  async function createDocument() {
    if (!form.documentPath.trim()) return
    if (await workspace.createDocument(form.documentPath.trim())) closeModal()
  }

  function openWorktreeDialog() {
    const repository = workspace.activeRepository.value
    if (!repository) return
    form.worktreeName = ''
    form.worktreeBranch = ''
    form.worktreeStart = workspace.activeWorktree.value?.branch ?? 'HEAD'
    form.worktreePath = ''
    modal.value = 'worktree'
  }

  watch(
    () => form.worktreeName,
    (name) => {
      const repository = workspace.activeRepository.value
      if (!repository || !name.trim()) return
      const separator = repository.root.includes('\\') ? '\\' : '/'
      const parent = repository.root.replace(/[\\/][^\\/]+$/, '')
      form.worktreePath = `${parent}${separator}${repository.name}-${name.trim()}`
      if (!form.worktreeBranch) form.worktreeBranch = name.trim()
    },
  )

  async function createWorktree() {
    const repository = workspace.activeRepository.value
    if (!repository) return
    try {
      await nativeApi.createWorktree({
        request: {
          root: repository.root,
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
        url: `/?root=${encodeURIComponent(worktree.path)}`,
        title: `${workspace.activeRepository.value?.name ?? 'Marktree'} · ${worktree.name}`,
        width: 1280,
        height: 820,
        minWidth: 900,
        minHeight: 600,
      })
    } catch (reason) {
      workspace.error.value = readableError(reason)
    }
  }

  return {
    chooseRepository,
    chooseCloneDestination,
    openCloneDialog,
    cloneRepository,
    initializeMobileRepository,
    openNewDocument,
    createDocument,
    openWorktreeDialog,
    createWorktree,
    openWorktreeWindow,
  }
}
