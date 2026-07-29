import { openUrl } from '@tauri-apps/plugin-opener'
import { onBeforeUnmount, ref, type Ref } from 'vue'

import type { useWorkspace } from '@/composables/useWorkspace'
import { i18n } from '@/i18n'
import { isTauri, nativeApi } from '@/lib/api'
import { readableError } from '@/lib/errors'
import type { AuthConfiguration, GithubDeviceCode } from '@/types'

import type { AppModal, RepositoryDialogForm } from './dialogState'

type Workspace = ReturnType<typeof useWorkspace>
type GithubCredentialTarget =
  | { kind: 'clone' }
  | { kind: 'repository'; repositoryId: string; root: string }

export function useRepositorySettings(
  workspace: Workspace,
  modal: Ref<AppModal | undefined>,
  form: RepositoryDialogForm,
  clearRepositoryTimers: (roots: Iterable<string>) => void,
) {
  const authConfiguration = ref<AuthConfiguration>()
  const githubDevice = ref<GithubDeviceCode>()
  const githubPending = ref(false)
  const cloneCredentialId = ref<string>()
  const repositoryConfigSha256 = ref<string | null>(null)
  const repositoryConfigMissing = ref(true)
  const repositoryConfigRoot = ref<string>()
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
      workspace.error.value = readableError(reason)
    }
  }

  async function openCredentials() {
    modal.value = 'credentials'
    form.credentialUsername = ''
    form.credentialToken = ''
    githubDevice.value = undefined
    repositoryConfigRoot.value = workspace.activeRoot.value
    repositoryConfigSha256.value = null
    repositoryConfigMissing.value = true
    try {
      await loadAuthConfiguration()
      if (isTauri() && workspace.activeRoot.value) {
        const snapshot = await nativeApi.readRepositoryConfig({
          root: workspace.activeRoot.value,
        })
        form.assetsDir = snapshot.config.assetsDir
        form.ignoreRules = snapshot.config.ignoreRules.join('\n')
        repositoryConfigSha256.value = snapshot.sha256
        repositoryConfigMissing.value = snapshot.missing
      }
    } catch (reason) {
      workspace.error.value = readableError(reason)
    }
  }

  async function saveGenericCredential() {
    const repository = workspace.activeRepository.value
    const root = workspace.activeRoot.value
    if (!repository || !root || !form.credentialToken.trim()) return
    const id = `repository-${repository.id}`
    try {
      await nativeApi.saveCredential({
        input: {
          id,
          username: form.credentialUsername.trim(),
          token: form.credentialToken.trim(),
        },
      })
      await nativeApi.setRepositoryCredential({ root, credentialId: id })
      workspace.message.value = i18n.global.t('app.credentialSaved')
      closeModal()
    } catch (reason) {
      workspace.error.value = readableError(reason)
    }
  }

  async function saveRepositoryConfig() {
    const root = repositoryConfigRoot.value
    if (!root) return
    try {
      const snapshot = await nativeApi.saveRepositoryConfig({
        request: {
          root,
          config: {
            assetsDir: form.assetsDir.trim() || 'assets',
            ignoreRules: form.ignoreRules
              .split(/\r?\n/)
              .map((rule) => rule.trim())
              .filter(Boolean),
          },
          expectedSha256: repositoryConfigSha256.value,
          expectedMissing: repositoryConfigMissing.value,
        },
      })
      repositoryConfigSha256.value = snapshot.sha256
      repositoryConfigMissing.value = snapshot.missing
      workspace.message.value = i18n.global.t('app.settingsSaved')
      await workspace.loadDocuments()
    } catch (reason) {
      workspace.error.value = readableError(reason)
    }
  }

  async function forgetActiveRepository() {
    if (!window.confirm(i18n.global.t('app.forgetRepositoryConfirm'))) return
    const roots =
      workspace.activeRepository.value?.worktrees.map((worktree) => worktree.path) ?? []
    clearRepositoryTimers(roots)
    try {
      await workspace.forgetActiveRepository()
      closeModal()
    } catch (reason) {
      workspace.error.value = readableError(reason)
    }
  }

  async function beginGithubLogin() {
    const target: GithubCredentialTarget =
      modal.value === 'clone'
        ? { kind: 'clone' }
        : workspace.activeRepository.value && workspace.activeRoot.value
          ? {
              kind: 'repository',
              repositoryId: workspace.activeRepository.value.id,
              root: workspace.activeRoot.value,
            }
          : { kind: 'clone' }
    try {
      githubDevice.value = await nativeApi.beginGithubDeviceFlow()
      await openUrl(githubDevice.value.verificationUri)
      githubPending.value = true
      scheduleGithubPoll(target)
    } catch (reason) {
      workspace.error.value = readableError(reason)
    }
  }

  function scheduleGithubPoll(target: GithubCredentialTarget) {
    if (!githubDevice.value) return
    const delay = Math.max(githubDevice.value.interval, 5) * 1000
    githubPollTimer = window.setTimeout(async () => {
      const expectedModal = target.kind === 'clone' ? 'clone' : 'credentials'
      if (!githubDevice.value || modal.value !== expectedModal) return
      try {
        const token = await nativeApi.pollGithubDeviceFlow({
          deviceCode: githubDevice.value.deviceCode,
        })
        if (token.accessToken) {
          const id =
            target.kind === 'clone'
              ? (cloneCredentialId.value ??= `clone-${crypto.randomUUID()}`)
              : `github-${target.repositoryId}`
          await nativeApi.saveCredential({
            input: { id, username: 'x-access-token', token: token.accessToken },
          })
          githubPending.value = false
          githubDevice.value = undefined
          if (target.kind === 'clone') {
            workspace.message.value = i18n.global.t('app.githubCredentialReady')
          } else {
            await nativeApi.setRepositoryCredential({
              root: target.root,
              credentialId: id,
            })
            workspace.message.value = i18n.global.t('app.githubConnected')
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
        workspace.error.value = readableError(reason)
      }
    }, delay)
  }

  function closeModal() {
    if (githubPollTimer) window.clearTimeout(githubPollTimer)
    githubPollTimer = undefined
    githubPending.value = false
    modal.value = undefined
  }

  onBeforeUnmount(() => {
    if (githubPollTimer) window.clearTimeout(githubPollTimer)
  })

  return {
    authConfiguration,
    githubDevice,
    githubPending,
    cloneCredentialId,
    repositoryConfigSha256,
    repositoryConfigMissing,
    prepareCloneCredentials,
    openCredentials,
    saveGenericCredential,
    saveRepositoryConfig,
    forgetActiveRepository,
    beginGithubLogin,
    closeModal,
  }
}
