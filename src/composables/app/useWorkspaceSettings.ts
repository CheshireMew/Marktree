import { openUrl } from '@tauri-apps/plugin-opener'
import { onBeforeUnmount, ref, type Ref } from 'vue'

import type { WorkspaceApi } from '@/composables/useWorkspace'
import { i18n } from '@/i18n'
import { isTauri, nativeApi } from '@/lib/api'
import type {
  AuthConfiguration,
  GithubDeviceCode,
  OperationLogEntry,
} from '@/types'

import type { AppModal, WorkspaceDialogForm } from './dialogState'

type Workspace = Pick<
  WorkspaceApi,
  | 'activeRoot'
  | 'activeWorkspace'
  | 'forgetActiveWorkspace'
  | 'loadDocuments'
  | 'loadWorkspaceTrash'
  | 'notify'
  | 'reportError'
>
type GithubCredentialTarget =
  | { kind: 'clone' }
  | { kind: 'workspace'; workspaceId: string; root: string }

export function useWorkspaceSettings(
  workspace: Workspace,
  modal: Ref<AppModal | undefined>,
  form: WorkspaceDialogForm,
  clearWorkspaceTimers: (roots: Iterable<string>) => void,
  openDialog: (value: AppModal) => void,
  runDialogAction: <T>(action: () => Promise<T>) => Promise<T | undefined>,
  closeDialog: () => void,
) {
  const authConfiguration = ref<AuthConfiguration>()
  const githubDevice = ref<GithubDeviceCode>()
  const githubPending = ref(false)
  const cloneCredentialId = ref<string>()
  const workspaceConfigSha256 = ref<string | null>(null)
  const workspaceConfigMissing = ref(true)
  const workspaceConfigRoot = ref<string>()
  const operationLog = ref<OperationLogEntry[]>([])
  let githubPollTimer: number | undefined

  async function loadAuthConfiguration() {
    if (!isTauri()) return
    authConfiguration.value = await nativeApi.authConfiguration()
  }

  async function prepareCloneCredentials() {
    form.credentialUsername = ''
    form.credentialToken = ''
    cloneCredentialId.value = undefined
    githubDevice.value = undefined
    try {
      await loadAuthConfiguration()
    } catch (reason) {
      workspace.reportError(reason)
    }
  }

  async function openSettings() {
    openDialog('settings')
    form.credentialUsername = ''
    form.credentialToken = ''
    githubDevice.value = undefined
    workspaceConfigRoot.value = workspace.activeRoot.value
    workspaceConfigSha256.value = null
    workspaceConfigMissing.value = true
    operationLog.value = []
    await runDialogAction(async () => {
      await loadAuthConfiguration()
      if (isTauri()) {
        operationLog.value = await nativeApi.readOperationLog({ limit: 50 })
      }
      if (isTauri() && workspace.activeRoot.value) {
        await workspace.loadWorkspaceTrash()
        const snapshot = await nativeApi.readWorkspaceConfig({
          root: workspace.activeRoot.value,
        })
        form.assetsDir = snapshot.config.assetsDir
        form.ignoreRules = snapshot.config.ignoreRules.join('\n')
        workspaceConfigSha256.value = snapshot.sha256
        workspaceConfigMissing.value = snapshot.missing
      }
      return true
    })
  }

  async function saveGenericCredential() {
    const activeWorkspace = workspace.activeWorkspace.value
    const root = workspace.activeRoot.value
    if (!activeWorkspace?.git || !root || !form.credentialToken.trim()) return
    const id = `workspace-${activeWorkspace.id}`
    const saved = await runDialogAction(async () => {
      await nativeApi.saveCredential({
        input: {
          id,
          username: form.credentialUsername.trim(),
          token: form.credentialToken.trim(),
        },
      })
      await nativeApi.setWorkspaceGitCredential({ root, credentialId: id })
      workspace.notify(i18n.global.t('app.credentialSaved'))
      return true
    })
    if (saved) closeModal()
  }

  async function saveWorkspaceConfig() {
    const root = workspaceConfigRoot.value
    if (!root) return
    await runDialogAction(async () => {
      const snapshot = await nativeApi.saveWorkspaceConfig({
        request: {
          root,
          config: {
            assetsDir: form.assetsDir.trim() || 'assets',
            ignoreRules: form.ignoreRules
              .split(/\r?\n/)
              .map((rule) => rule.trim())
              .filter(Boolean),
          },
          expectedSha256: workspaceConfigSha256.value,
          expectedMissing: workspaceConfigMissing.value,
        },
      })
      workspaceConfigSha256.value = snapshot.sha256
      workspaceConfigMissing.value = snapshot.missing
      workspace.notify(i18n.global.t('app.settingsSaved'))
      await workspace.loadDocuments()
      return true
    })
  }

  async function forgetActiveWorkspace(): Promise<boolean> {
    const roots =
      workspace.activeWorkspace.value?.git?.worktrees.map((worktree) => worktree.path) ??
      (workspace.activeWorkspace.value ? [workspace.activeWorkspace.value.root] : [])
    clearWorkspaceTimers(roots)
    try {
      await workspace.forgetActiveWorkspace()
      return true
    } catch (reason) {
      workspace.reportError(reason)
      return false
    }
  }

  async function beginGithubLogin() {
    if (githubPending.value) return
    githubPending.value = true
    const target: GithubCredentialTarget =
      modal.value === 'clone'
        ? { kind: 'clone' }
        : workspace.activeWorkspace.value?.git && workspace.activeRoot.value
          ? {
              kind: 'workspace',
              workspaceId: workspace.activeWorkspace.value.id,
              root: workspace.activeRoot.value,
            }
          : { kind: 'clone' }
    try {
      githubDevice.value = await nativeApi.beginGithubDeviceFlow()
      await openUrl(githubDevice.value.verificationUri)
      scheduleGithubPoll(target)
    } catch (reason) {
      githubPending.value = false
      workspace.reportError(reason)
    }
  }

  function scheduleGithubPoll(target: GithubCredentialTarget) {
    if (!githubDevice.value) return
    const delay = Math.max(githubDevice.value.interval, 5) * 1000
    githubPollTimer = window.setTimeout(async () => {
      const expectedModal = target.kind === 'clone' ? 'clone' : 'settings'
      if (!githubDevice.value || modal.value !== expectedModal) return
      try {
        const token = await nativeApi.pollGithubDeviceFlow({
          deviceCode: githubDevice.value.deviceCode,
        })
        if (token.accessToken) {
          const id =
            target.kind === 'clone'
              ? (cloneCredentialId.value ??= `clone-${crypto.randomUUID()}`)
              : `github-${target.workspaceId}`
          await nativeApi.saveCredential({
            input: { id, username: 'x-access-token', token: token.accessToken },
          })
          githubPending.value = false
          githubDevice.value = undefined
          if (target.kind === 'clone') {
            workspace.notify(i18n.global.t('app.githubCredentialReady'))
          } else {
            await nativeApi.setWorkspaceGitCredential({
              root: target.root,
              credentialId: id,
            })
            workspace.notify(i18n.global.t('app.githubConnected'))
            closeModal()
          }
        } else if (token.pending) {
          scheduleGithubPoll(target)
        } else {
          githubPending.value = false
          throw new Error(
            token.error ?? i18n.global.t('app.githubAuthorizationFailed'),
          )
        }
      } catch (reason) {
        githubPending.value = false
        workspace.reportError(reason)
      }
    }, delay)
  }

  function closeModal() {
    if (githubPollTimer) window.clearTimeout(githubPollTimer)
    githubPollTimer = undefined
    githubPending.value = false
    closeDialog()
  }

  onBeforeUnmount(() => {
    if (githubPollTimer) window.clearTimeout(githubPollTimer)
  })

  return {
    authConfiguration,
    githubDevice,
    githubPending,
    cloneCredentialId,
    workspaceConfigSha256,
    workspaceConfigMissing,
    operationLog,
    prepareCloneCredentials,
    openSettings,
    saveGenericCredential,
    saveWorkspaceConfig,
    forgetActiveWorkspace,
    beginGithubLogin,
    closeModal,
  }
}
