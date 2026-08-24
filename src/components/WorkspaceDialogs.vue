<script setup lang="ts">
import { computed, ref } from 'vue'

import CredentialFields from '@/components/CredentialFields.vue'
import WorkspaceSettingsDialog from '@/components/WorkspaceSettingsDialog.vue'
import { useDialogAccessibility } from '@/composables/useDialogAccessibility'
import type {
  AuthConfiguration,
  GithubDeviceCode,
  OperationLogEntry,
  TrashEntry,
} from '@/types'
import type {
  AppModal,
  ConfirmationDialogState,
  WorkspaceDialogForm,
} from '@/composables/app/dialogState'

const props = defineProps<{
  modal?: AppModal
  form: WorkspaceDialogForm
  nativeAndroid: boolean
  gitEnabled: boolean
  authConfiguration?: AuthConfiguration
  githubDevice?: GithubDeviceCode
  githubPending: boolean
  cloneCredentialId?: string
  trashEntries: TrashEntry[]
  operationLog: OperationLogEntry[]
  busy?: boolean
  error?: string
  confirmation?: ConfirmationDialogState
}>()

const emit = defineEmits<{
  close: []
  chooseCloneDestination: []
  clone: []
  beginGithubLogin: []
  createDocument: []
  createMobileWorkspace: []
  createDesktopWorkspace: []
  createWorktree: []
  submitEntryAction: []
  saveGenericCredential: []
  saveWorkspaceConfig: []
  enableGit: []
  forgetActiveWorkspace: []
  restoreTrash: [id: string]
  emptyTrash: []
  exportWorkspaceArchive: []
  confirm: []
}>()

const dialogSurface = ref<HTMLElement>()
useDialogAccessibility(computed(() => props.modal), dialogSurface, () => emit('close'))
</script>

<template>
  <div
    v-if="modal"
    ref="dialogSurface"
    class="modal-backdrop"
    @mousedown.self="$emit('close')"
  >
    <form
      v-if="modal === 'clone'"
      class="dialog"
      role="dialog"
      aria-modal="true"
      aria-labelledby="workspace-dialog-title"
      tabindex="-1"
      @submit.prevent="$emit('clone')"
    >
      <header>
        <h2 id="workspace-dialog-title">{{ $t('app.clone') }}</h2>
        <button type="button" :disabled="busy" :aria-label="$t('app.close')" @click="$emit('close')">×</button>
      </header>
      <label>
        <span>{{ $t('app.httpsUrl') }}</span>
        <input
          v-model="form.remoteUrl"
          autofocus
          :disabled="busy"
          placeholder="https://github.com/user/repository.git"
        />
      </label>
      <label v-if="nativeAndroid">
        <span>{{ $t('app.workspaceName') }}</span>
        <input v-model="form.workspaceName" :disabled="busy" placeholder="my-notes" />
      </label>
      <label v-else>
        <span>{{ $t('app.localFolder') }}</span>
        <div class="input-action">
          <input v-model="form.destination" :disabled="busy" placeholder="D:\Documents\notes" />
          <button type="button" :disabled="busy" :aria-label="$t('app.chooseFolder')" @click="$emit('chooseCloneDestination')">…</button>
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
            :disabled="busy || githubPending"
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
        <CredentialFields
          v-model:username="form.credentialUsername"
          v-model:token="form.credentialToken"
          :disabled="busy"
        />
      </section>
      <p v-if="error" class="dialog-feedback error" role="alert">{{ error }}</p>
      <footer>
        <button type="button" :disabled="busy" @click="$emit('close')">{{ $t('app.close') }}</button>
        <button class="primary" :disabled="busy || !form.remoteUrl.trim() || (!nativeAndroid && !form.destination.trim())">{{ busy ? $t('app.working') : $t('app.clone') }}</button>
      </footer>
    </form>

    <form
      v-else-if="modal === 'document'"
      class="dialog compact-dialog"
      role="dialog"
      aria-modal="true"
      aria-labelledby="workspace-dialog-title"
      tabindex="-1"
      @submit.prevent="$emit('createDocument')"
    >
      <header>
        <h2 id="workspace-dialog-title">{{ $t('app.newDocument') }}</h2>
        <button type="button" :disabled="busy" :aria-label="$t('app.close')" @click="$emit('close')">×</button>
      </header>
      <label>
        <span>{{ $t('app.path') }}</span>
        <input
          v-model="form.documentPath"
          autofocus
          :disabled="busy"
          placeholder="notes/new-document.md"
        />
      </label>
      <p v-if="error" class="dialog-feedback error" role="alert">{{ error }}</p>
      <footer>
        <button type="button" :disabled="busy" @click="$emit('close')">{{ $t('app.close') }}</button>
        <button class="primary" :disabled="busy || !form.documentPath.trim()">{{ busy ? $t('app.working') : $t('app.newDocument') }}</button>
      </footer>
    </form>

    <form
      v-else-if="modal === 'mobileWorkspace'"
      class="dialog compact-dialog"
      role="dialog"
      aria-modal="true"
      aria-labelledby="workspace-dialog-title"
      tabindex="-1"
      @submit.prevent="$emit('createMobileWorkspace')"
    >
      <header>
        <h2 id="workspace-dialog-title">{{ $t('app.newFolder') }}</h2>
        <button type="button" :disabled="busy" :aria-label="$t('app.close')" @click="$emit('close')">×</button>
      </header>
      <label>
        <span>{{ $t('app.workspaceName') }}</span>
        <input v-model="form.workspaceName" autofocus :disabled="busy" placeholder="my-notes" />
      </label>
      <p v-if="error" class="dialog-feedback error" role="alert">{{ error }}</p>
      <footer>
        <button type="button" :disabled="busy" @click="$emit('close')">{{ $t('app.close') }}</button>
        <button class="primary" :disabled="busy || !form.workspaceName.trim()">{{ busy ? $t('app.working') : $t('app.create') }}</button>
      </footer>
    </form>

    <form
      v-else-if="modal === 'desktopWorkspace'"
      class="dialog compact-dialog"
      role="dialog"
      aria-modal="true"
      aria-labelledby="workspace-dialog-title"
      tabindex="-1"
      @submit.prevent="$emit('createDesktopWorkspace')"
    >
      <header>
        <h2 id="workspace-dialog-title">{{ $t('app.newFolder') }}</h2>
        <button type="button" :disabled="busy" :aria-label="$t('app.close')" @click="$emit('close')">×</button>
      </header>
      <label>
        <span>{{ $t('app.workspaceName') }}</span>
        <input v-model="form.workspaceName" autofocus :disabled="busy" placeholder="my-notes" />
      </label>
      <p class="entry-action-summary">{{ form.destination }}</p>
      <p v-if="error" class="dialog-feedback error" role="alert">{{ error }}</p>
      <footer>
        <button type="button" :disabled="busy" @click="$emit('close')">{{ $t('app.close') }}</button>
        <button class="primary" :disabled="busy || !form.workspaceName.trim()">{{ busy ? $t('app.working') : $t('app.create') }}</button>
      </footer>
    </form>

    <form
      v-else-if="modal === 'worktree'"
      class="dialog"
      role="dialog"
      aria-modal="true"
      aria-labelledby="workspace-dialog-title"
      tabindex="-1"
      @submit.prevent="$emit('createWorktree')"
    >
      <header>
        <h2 id="workspace-dialog-title">{{ $t('app.worktreeNew') }}</h2>
        <button type="button" :disabled="busy" :aria-label="$t('app.close')" @click="$emit('close')">×</button>
      </header>
      <div class="form-grid">
        <label>
          <span>{{ $t('app.fieldName') }}</span>
          <input v-model="form.worktreeName" autofocus :disabled="busy" placeholder="book" />
        </label>
        <label>
          <span>{{ $t('app.branch') }}</span>
          <input v-model="form.worktreeBranch" :disabled="busy" placeholder="book" />
        </label>
      </div>
      <label>
        <span>{{ $t('app.startFrom') }}</span>
        <input v-model="form.worktreeStart" :disabled="busy" :placeholder="$t('app.headOrBranch')" />
      </label>
      <label>
        <span>{{ $t('app.folder') }}</span>
        <input v-model="form.worktreePath" :disabled="busy" />
      </label>
      <p v-if="error" class="dialog-feedback error" role="alert">{{ error }}</p>
      <footer>
        <button type="button" :disabled="busy" @click="$emit('close')">{{ $t('app.close') }}</button>
        <button class="primary" :disabled="busy || !form.worktreeName.trim() || !form.worktreePath.trim() || !form.worktreeBranch.trim()">{{ busy ? $t('app.working') : $t('app.worktreeNew') }}</button>
      </footer>
    </form>

    <form
      v-else-if="modal === 'workspaceEntry'"
      class="dialog compact-dialog"
      role="dialog"
      aria-modal="true"
      aria-labelledby="workspace-dialog-title"
      tabindex="-1"
      @submit.prevent="$emit('submitEntryAction')"
    >
      <header>
        <h2 id="workspace-dialog-title">
          {{
            form.entryAction === 'newFolder'
              ? $t('app.newFolder')
              : form.entryAction === 'rename'
                ? $t('app.rename')
                : form.entryAction === 'duplicate'
                  ? $t('app.createDuplicate')
                  : $t('app.moveToTrash')
          }}
        </h2>
        <button type="button" :disabled="busy" :aria-label="$t('app.close')" @click="$emit('close')">×</button>
      </header>
      <p v-if="form.entryAction === 'trash'" class="entry-action-summary">
        {{ $t('app.trashEntryConfirm', { name: form.entryOriginalName }) }}
      </p>
      <label v-else>
        <span>{{ form.entryAction === 'newFolder' ? $t('app.folder') : $t('app.fieldName') }}</span>
        <input v-model="form.entryName" autofocus :disabled="busy" />
      </label>
      <p v-if="error" class="dialog-feedback error" role="alert">{{ error }}</p>
      <footer>
        <button type="button" :disabled="busy" @click="$emit('close')">{{ $t('app.close') }}</button>
        <button
          class="primary"
          :class="{ danger: form.entryAction === 'trash' }"
          :disabled="busy || (form.entryAction !== 'trash' && (!form.entryName.trim() || (form.entryAction === 'rename' && form.entryName.trim() === form.entryOriginalName)))"
        >
          {{ busy ? $t('app.working') : form.entryAction === 'trash' ? $t('app.moveToTrash') : $t('app.confirm') }}
        </button>
      </footer>
    </form>

    <section
      v-else-if="modal === 'confirmation' && confirmation"
      class="dialog compact-dialog confirmation-dialog"
      role="alertdialog"
      aria-modal="true"
      aria-labelledby="workspace-dialog-title"
      tabindex="-1"
    >
      <header>
        <h2 id="workspace-dialog-title">{{ confirmation.title }}</h2>
        <button type="button" :disabled="busy" :aria-label="$t('app.close')" @click="$emit('close')">×</button>
      </header>
      <p class="entry-action-summary">{{ confirmation.message }}</p>
      <p v-if="error" class="dialog-feedback error" role="alert">{{ error }}</p>
      <footer>
        <button type="button" :disabled="busy" @click="$emit('close')">{{ $t('app.close') }}</button>
        <button
          class="primary"
          :class="{ danger: confirmation.danger }"
          :disabled="busy"
          @click="$emit('confirm')"
        >
          {{ busy ? $t('app.working') : confirmation.confirmLabel }}
        </button>
      </footer>
    </section>

    <WorkspaceSettingsDialog
      v-else
      :form="form"
      :native-android="nativeAndroid"
      :git-enabled="gitEnabled"
      :auth-configuration="authConfiguration"
      :github-device="githubDevice"
      :github-pending="githubPending"
      :trash-entries="trashEntries"
      :operation-log="operationLog"
      :busy="busy"
      :error="error"
      @close="$emit('close')"
      @begin-github-login="$emit('beginGithubLogin')"
      @save-generic-credential="$emit('saveGenericCredential')"
      @save-workspace-config="$emit('saveWorkspaceConfig')"
      @enable-git="$emit('enableGit')"
      @forget-active-workspace="$emit('forgetActiveWorkspace')"
      @restore-trash="$emit('restoreTrash', $event)"
      @empty-trash="$emit('emptyTrash')"
      @export-workspace-archive="$emit('exportWorkspaceArchive')"
    />
  </div>
</template>
