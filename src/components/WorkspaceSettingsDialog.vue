<script setup lang="ts">
import CredentialFields from '@/components/CredentialFields.vue'
import EditorPreferencesPanel from '@/components/EditorPreferencesPanel.vue'
import type {
  AuthConfiguration,
  GithubDeviceCode,
  OperationLogEntry,
  TrashEntry,
} from '@/types'

defineProps<{
  nativeAndroid: boolean
  gitEnabled: boolean
  authConfiguration?: AuthConfiguration
  githubDevice?: GithubDeviceCode
  githubPending: boolean
  form: {
    credentialUsername: string
    credentialToken: string
    assetsDir: string
    ignoreRules: string
  }
  trashEntries: TrashEntry[]
  operationLog: OperationLogEntry[]
  busy?: boolean
  error?: string
}>()

defineEmits<{
  close: []
  beginGithubLogin: []
  saveGenericCredential: []
  saveWorkspaceConfig: []
  enableGit: []
  forgetActiveWorkspace: []
  restoreTrash: [id: string]
  emptyTrash: []
  exportWorkspaceArchive: []
}>()
</script>

<template>
  <section
    class="dialog credentials-dialog"
    role="dialog"
    aria-modal="true"
    aria-labelledby="workspace-dialog-title"
    tabindex="-1"
  >
    <header>
      <div>
        <h2 id="workspace-dialog-title">{{ $t('app.workspaceSettings') }}</h2>
        <p>{{ $t('app.workspaceSettingsHint') }}</p>
      </div>
      <button type="button" :disabled="busy" :aria-label="$t('app.close')" @click="$emit('close')">×</button>
    </header>
    <p v-if="error" class="dialog-feedback error" role="alert">{{ error }}</p>
    <p v-else-if="busy" class="dialog-feedback" role="status">{{ $t('app.working') }}</p>
    <section v-if="gitEnabled" class="github-auth">
      <div>
        <strong>GitHub</strong>
        <span>{{ $t('app.authorizeDevice') }}</span>
      </div>
      <button
        v-if="authConfiguration?.githubEnabled"
        class="primary"
        :disabled="busy || githubPending"
        @click="$emit('beginGithubLogin')"
      >
        {{ githubPending ? $t('app.waiting') : $t('app.connectGithub') }}
      </button>
      <span v-else class="auth-unavailable">{{ $t('app.githubUnavailable') }}</span>
    </section>
    <section v-else class="version-management">
      <div>
        <strong>{{ $t('app.enableGit') }}</strong>
        <span>{{ $t('app.enableGitHint') }}</span>
      </div>
      <button class="primary enable-git-button" :disabled="busy" @click="$emit('enableGit')">
        {{ $t('app.enableGit') }}
      </button>
    </section>
    <div v-if="gitEnabled && githubDevice" class="device-code">
      <span>{{ $t('app.githubCode') }}</span>
      <strong>{{ githubDevice.userCode }}</strong>
    </div>
    <form v-if="gitEnabled" class="token-form" @submit.prevent="$emit('saveGenericCredential')">
      <h3>{{ $t('app.otherHttpsRemote') }}</h3>
      <CredentialFields
        v-model:username="form.credentialUsername"
        v-model:token="form.credentialToken"
        :disabled="busy"
      />
      <footer>
        <button class="primary" :disabled="busy || !form.credentialToken.trim()">{{ $t('app.saveCredential') }}</button>
      </footer>
    </form>
    <form class="token-form workspace-config-form" @submit.prevent="$emit('saveWorkspaceConfig')">
      <h3>{{ $t('app.workspaceSettings') }}</h3>
      <label>
        <span>{{ $t('app.assetFolder') }}</span>
        <input v-model="form.assetsDir" :disabled="busy" placeholder="assets" />
      </label>
      <label>
        <span>{{ $t('app.ignoreRules') }}</span>
        <textarea v-model="form.ignoreRules" :disabled="busy" placeholder="build/**&#10;private/**" />
      </label>
      <footer>
        <button class="primary" :disabled="busy">{{ $t('app.saveSettings') }}</button>
      </footer>
    </form>
    <EditorPreferencesPanel />
    <section v-if="nativeAndroid" class="workspace-trash">
      <header>
        <h3>{{ $t('app.trash') }}</h3>
        <button v-if="trashEntries.length" class="danger" :disabled="busy" @click="$emit('emptyTrash')">
          {{ $t('app.emptyTrash') }}
        </button>
      </header>
      <p v-if="!trashEntries.length">{{ $t('app.trashEmpty') }}</p>
      <div v-for="entry in trashEntries" :key="entry.id" class="trash-row">
        <span>
          <strong>{{ entry.name }}</strong>
          <small>{{ entry.originalPath }}</small>
        </span>
        <button :disabled="busy" @click="$emit('restoreTrash', entry.id)">{{ $t('app.restore') }}</button>
      </div>
    </section>
    <section v-if="nativeAndroid" class="version-management workspace-portability">
      <div>
        <strong>{{ $t('app.workspaceArchive') }}</strong>
        <span>{{ $t('app.workspaceArchiveHint') }}</span>
      </div>
      <button class="primary" :disabled="busy" @click="$emit('exportWorkspaceArchive')">
        {{ $t('app.exportWorkspaceArchive') }}
      </button>
    </section>
    <details class="operation-log">
      <summary>
        <span>
          <strong>{{ $t('app.operationLog') }}</strong>
          <small>{{ $t('app.operationLogHint') }}</small>
        </span>
        <i>{{ operationLog.length }}</i>
      </summary>
      <p v-if="!operationLog.length">{{ $t('app.operationLogEmpty') }}</p>
      <ol v-else>
        <li v-for="entry in operationLog" :key="`${entry.timestamp}-${entry.operationId}`">
          <span>
            <strong>{{ $t(`app.operationAction.${entry.action}`) }}</strong>
            <small>{{ entry.root }}</small>
          </span>
          <span>
            <time :datetime="entry.timestamp">{{ new Date(entry.timestamp).toLocaleString() }}</time>
            <em :class="entry.outcome">{{ $t(`app.operationOutcome.${entry.outcome}`) }}</em>
            <small>{{ $t(`app.operationPhase.${entry.phase}`) }}</small>
          </span>
        </li>
      </ol>
    </details>
    <footer class="workspace-lifecycle">
      <button class="danger" :disabled="busy" @click="$emit('forgetActiveWorkspace')">
        {{ $t('app.forgetWorkspace') }}
      </button>
      <span>{{ $t('app.forgetWorkspaceHint') }}</span>
    </footer>
  </section>
</template>
