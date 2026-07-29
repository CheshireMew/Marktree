<script setup lang="ts">
import type {
  AuthConfiguration,
  GithubDeviceCode,
} from '@/types'
import type {
  AppModal,
  RepositoryDialogForm,
} from '@/composables/app/dialogState'

defineProps<{
  modal?: AppModal
  form: RepositoryDialogForm
  nativeAndroid: boolean
  authConfiguration?: AuthConfiguration
  githubDevice?: GithubDeviceCode
  githubPending: boolean
  cloneCredentialId?: string
}>()

defineEmits<{
  close: []
  chooseCloneDestination: []
  clone: []
  beginGithubLogin: []
  createDocument: []
  initializeMobileRepository: []
  createWorktree: []
  saveGenericCredential: []
  saveRepositoryConfig: []
  forgetActiveRepository: []
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
        <span>{{ $t('app.repositoryName') }}</span>
        <input v-model="form.repositoryName" placeholder="my-notes" />
      </label>
      <label v-else>
        <span>{{ $t('app.localFolder') }}</span>
        <div class="input-action">
          <input v-model="form.destination" placeholder="D:\Documents\repository" />
          <button type="button" @click="$emit('chooseCloneDestination')">…</button>
        </div>
      </label>
      <section class="clone-auth">
        <header>
          <div>
            <strong>{{ $t('app.privateRepository') }}</strong>
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
      v-else-if="modal === 'mobileRepository'"
      class="dialog compact-dialog"
      @submit.prevent="$emit('initializeMobileRepository')"
    >
      <header>
        <h2>{{ $t('app.initialize') }}</h2>
        <button type="button" @click="$emit('close')">×</button>
      </header>
      <label>
        <span>{{ $t('app.repositoryName') }}</span>
        <input v-model="form.repositoryName" autofocus placeholder="my-notes" />
      </label>
      <footer>
        <button type="button" @click="$emit('close')">{{ $t('app.close') }}</button>
        <button class="primary">{{ $t('app.initialize') }}</button>
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
          <h2>{{ $t('app.remoteCredentials') }}</h2>
          <p>{{ $t('app.credentialStorageHint') }}</p>
        </div>
        <button type="button" @click="$emit('close')">×</button>
      </header>
      <section class="github-auth">
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
      <div v-if="githubDevice" class="device-code">
        <span>{{ $t('app.githubCode') }}</span>
        <strong>{{ githubDevice.userCode }}</strong>
      </div>
      <form class="token-form" @submit.prevent="$emit('saveGenericCredential')">
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
        class="token-form repository-config-form"
        @submit.prevent="$emit('saveRepositoryConfig')"
      >
        <h3>{{ $t('app.repositorySettings') }}</h3>
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
      <footer class="repository-lifecycle">
        <button class="danger" @click="$emit('forgetActiveRepository')">
          {{ $t('app.forgetRepository') }}
        </button>
        <span>{{ $t('app.forgetRepositoryHint') }}</span>
      </footer>
    </section>
  </div>
</template>
