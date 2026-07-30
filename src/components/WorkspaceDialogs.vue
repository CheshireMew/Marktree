<script setup lang="ts">
import type {
  AuthConfiguration,
  GithubDeviceCode,
  TrashEntry,
} from '@/types'
import type {
  AppModal,
  WorkspaceDialogForm,
} from '@/composables/app/dialogState'

defineProps<{
  modal?: AppModal
  form: WorkspaceDialogForm
  nativeAndroid: boolean
  gitEnabled: boolean
  authConfiguration?: AuthConfiguration
  githubDevice?: GithubDeviceCode
  githubPending: boolean
  cloneCredentialId?: string
  trashEntries: TrashEntry[]
}>()

defineEmits<{
  close: []
  chooseCloneDestination: []
  clone: []
  beginGithubLogin: []
  createDocument: []
  createMobileWorkspace: []
  createWorktree: []
  saveGenericCredential: []
  saveWorkspaceConfig: []
  forgetActiveWorkspace: []
  restoreTrash: [id: string]
  emptyTrash: []
}>()
</script>

<template>
  <div v-if="modal" class="modal-backdrop" @mousedown.self="$emit('close')">
    <form v-if="modal === 'clone'" class="dialog" @submit.prevent="$emit('clone')">
      <header>
        <h2>{{ $t('app.clone') }}</h2>
        <button type="button" @click="$emit('close')">×</button>
      </header>
      <label>
        <span>{{ $t('app.httpsUrl') }}</span>
        <input
          v-model="form.remoteUrl"
          autofocus
          placeholder="https://github.com/user/repository.git"
        />
      </label>
      <label v-if="nativeAndroid">
        <span>{{ $t('app.workspaceName') }}</span>
        <input v-model="form.workspaceName" placeholder="my-notes" />
      </label>
      <label v-else>
        <span>{{ $t('app.localFolder') }}</span>
        <div class="input-action">
          <input v-model="form.destination" placeholder="D:\Documents\notes" />
          <button type="button" @click="$emit('chooseCloneDestination')">…</button>
        </div>
      </label>
      <section class="clone-auth">
        <header>
          <div>
            <strong>{{ $t('app.privateGitRepository') }}</strong>
            <span>{{ $t('app.cloneCredentialHint') }}</span>
          </div>
          <button
            v-if="authConfiguration?.githubEnabled"
            type="button"
            :disabled="githubPending"
            @click="$emit('beginGithubLogin')"
          >
            {{
              cloneCredentialId
                ? $t('app.githubReady')
                : githubPending
                  ? $t('app.waiting')
                  : $t('app.connectGithub')
            }}
          </button>
        </header>
        <div v-if="githubDevice" class="device-code">
          <span>{{ $t('app.githubCode') }}</span>
          <strong>{{ githubDevice.userCode }}</strong>
        </div>
        <label>
          <span>{{ $t('app.username') }}</span>
          <input
            v-model="form.credentialUsername"
            autocomplete="username"
            placeholder="username"
          />
        </label>
        <label>
          <span>{{ $t('app.personalAccessToken') }}</span>
          <input
            v-model="form.credentialToken"
            type="password"
            autocomplete="current-password"
          />
        </label>
      </section>
      <footer>
        <button type="button" @click="$emit('close')">{{ $t('app.close') }}</button>
        <button class="primary">{{ $t('app.clone') }}</button>
      </footer>
    </form>

    <form
      v-else-if="modal === 'document'"
      class="dialog compact-dialog"
      @submit.prevent="$emit('createDocument')"
    >
      <header>
        <h2>{{ $t('app.newDocument') }}</h2>
        <button type="button" @click="$emit('close')">×</button>
      </header>
      <label>
        <span>{{ $t('app.path') }}</span>
        <input
          v-model="form.documentPath"
          autofocus
          placeholder="notes/new-document.md"
        />
      </label>
      <footer>
        <button type="button" @click="$emit('close')">{{ $t('app.close') }}</button>
        <button class="primary">{{ $t('app.newDocument') }}</button>
      </footer>
    </form>

    <form
      v-else-if="modal === 'mobileWorkspace'"
      class="dialog compact-dialog"
      @submit.prevent="$emit('createMobileWorkspace')"
    >
      <header>
        <h2>{{ $t('app.newFolder') }}</h2>
        <button type="button" @click="$emit('close')">×</button>
      </header>
      <label>
        <span>{{ $t('app.workspaceName') }}</span>
        <input v-model="form.workspaceName" autofocus placeholder="my-notes" />
      </label>
      <footer>
        <button type="button" @click="$emit('close')">{{ $t('app.close') }}</button>
        <button class="primary">{{ $t('app.create') }}</button>
      </footer>
    </form>

    <form
      v-else-if="modal === 'worktree'"
      class="dialog"
      @submit.prevent="$emit('createWorktree')"
    >
      <header>
        <h2>{{ $t('app.worktreeNew') }}</h2>
        <button type="button" @click="$emit('close')">×</button>
      </header>
      <div class="form-grid">
        <label>
          <span>{{ $t('app.fieldName') }}</span>
          <input v-model="form.worktreeName" autofocus placeholder="book" />
        </label>
        <label>
          <span>{{ $t('app.branch') }}</span>
          <input v-model="form.worktreeBranch" placeholder="book" />
        </label>
      </div>
      <label>
        <span>{{ $t('app.startFrom') }}</span>
        <input v-model="form.worktreeStart" :placeholder="$t('app.headOrBranch')" />
      </label>
      <label>
        <span>{{ $t('app.folder') }}</span>
        <input v-model="form.worktreePath" />
      </label>
      <footer>
        <button type="button" @click="$emit('close')">{{ $t('app.close') }}</button>
        <button class="primary">{{ $t('app.worktreeNew') }}</button>
      </footer>
    </form>

    <section v-else class="dialog credentials-dialog">
      <header>
        <div>
          <h2>{{ $t('app.workspaceSettings') }}</h2>
          <p>{{ $t('app.workspaceSettingsHint') }}</p>
        </div>
        <button type="button" @click="$emit('close')">×</button>
      </header>
      <section v-if="gitEnabled" class="github-auth">
        <div>
          <strong>GitHub</strong>
          <span>{{ $t('app.authorizeDevice') }}</span>
        </div>
        <button
          v-if="authConfiguration?.githubEnabled"
          class="primary"
          :disabled="githubPending"
          @click="$emit('beginGithubLogin')"
        >
          {{ githubPending ? $t('app.waiting') : $t('app.connectGithub') }}
        </button>
        <span v-else class="auth-unavailable">{{ $t('app.githubUnavailable') }}</span>
      </section>
      <div v-if="gitEnabled && githubDevice" class="device-code">
        <span>{{ $t('app.githubCode') }}</span>
        <strong>{{ githubDevice.userCode }}</strong>
      </div>
      <form v-if="gitEnabled" class="token-form" @submit.prevent="$emit('saveGenericCredential')">
        <h3>{{ $t('app.otherHttpsRemote') }}</h3>
        <label>
          <span>{{ $t('app.username') }}</span>
          <input
            v-model="form.credentialUsername"
            autocomplete="username"
            placeholder="username"
          />
        </label>
        <label>
          <span>{{ $t('app.personalAccessToken') }}</span>
          <input
            v-model="form.credentialToken"
            type="password"
            autocomplete="current-password"
          />
        </label>
        <footer>
          <button class="primary">{{ $t('app.saveCredential') }}</button>
        </footer>
      </form>
      <form
        class="token-form workspace-config-form"
        @submit.prevent="$emit('saveWorkspaceConfig')"
      >
        <h3>{{ $t('app.workspaceSettings') }}</h3>
        <label>
          <span>{{ $t('app.assetFolder') }}</span>
          <input v-model="form.assetsDir" placeholder="assets" />
        </label>
        <label>
          <span>{{ $t('app.ignoreRules') }}</span>
          <textarea v-model="form.ignoreRules" placeholder="build/**&#10;private/**" />
        </label>
        <footer>
          <button class="primary">{{ $t('app.saveSettings') }}</button>
        </footer>
      </form>
      <section v-if="nativeAndroid" class="workspace-trash">
        <header>
          <h3>{{ $t('app.trash') }}</h3>
          <button
            v-if="trashEntries.length"
            class="danger"
            @click="$emit('emptyTrash')"
          >
            {{ $t('app.emptyTrash') }}
          </button>
        </header>
        <p v-if="!trashEntries.length">{{ $t('app.trashEmpty') }}</p>
        <div v-for="entry in trashEntries" :key="entry.id" class="trash-row">
          <span>
            <strong>{{ entry.name }}</strong>
            <small>{{ entry.originalPath }}</small>
          </span>
          <button @click="$emit('restoreTrash', entry.id)">
            {{ $t('app.restore') }}
          </button>
        </div>
      </section>
      <footer class="workspace-lifecycle">
        <button class="danger" @click="$emit('forgetActiveWorkspace')">
          {{ $t('app.forgetWorkspace') }}
        </button>
        <span>{{ $t('app.forgetWorkspaceHint') }}</span>
      </footer>
    </section>
  </div>
</template>
