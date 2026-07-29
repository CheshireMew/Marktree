<script setup lang="ts">
import {
  Cloud,
  FolderGit2,
  GitCompareArrows,
  Menu,
  MoreHorizontal,
  Plus,
  RefreshCw,
  X,
} from 'lucide-vue-next'
import {
  computed,
  defineAsyncComponent,
  onBeforeUnmount,
  onMounted,
  ref,
  watch,
} from 'vue'
import { useI18n } from 'vue-i18n'

import ConflictDialog from '@/components/ConflictDialog.vue'
import DiffPanel from '@/components/DiffPanel.vue'
import GitPanel from '@/components/GitPanel.vue'
import RepositoryRail from '@/components/RepositoryRail.vue'
import RepositoryDialogs from '@/components/RepositoryDialogs.vue'
import WorkspaceSidebar from '@/components/WorkspaceSidebar.vue'
import WorkspaceOverlays from '@/components/WorkspaceOverlays.vue'
import { useDialogState } from '@/composables/app/dialogState'
import { useRepositoryProvisioning } from '@/composables/app/useRepositoryProvisioning'
import { useRepositorySettings } from '@/composables/app/useRepositorySettings'
import { useWorkspaceLifecycle } from '@/composables/app/useWorkspaceLifecycle'
import { useWorkspace } from '@/composables/useWorkspace'

const EditorWorkspace = defineAsyncComponent(
  () => import('@/components/EditorWorkspace.vue'),
)
const workspace = useWorkspace()
const { t } = useI18n()
const { modal, form } = useDialogState()

const dark = ref(localStorage.getItem('marktree-theme') === 'dark')
const sidebarOpen = ref(false)
const gitPanelOpen = ref(false)
const addMenuOpen = ref(false)
const quickOpen = ref(false)
const quickOpenQuery = ref('')

let closeActiveModal = () => {
  modal.value = undefined
}
const lifecycle = useWorkspaceLifecycle(workspace, () => {
  sidebarOpen.value = false
  if (modal.value && !['clone', 'mobileRepository'].includes(modal.value)) {
    closeActiveModal()
  }
})
const { viewportMobile, nativeAndroid, clearRepositoryTimers } = lifecycle

const settings = useRepositorySettings(
  workspace,
  modal,
  form,
  clearRepositoryTimers,
)
closeActiveModal = settings.closeModal
const {
  authConfiguration,
  githubDevice,
  githubPending,
  cloneCredentialId,
  openCredentials,
  saveGenericCredential,
  saveRepositoryConfig,
  forgetActiveRepository,
  beginGithubLogin,
  closeModal,
} = settings

const provisioning = useRepositoryProvisioning(
  workspace,
  modal,
  form,
  nativeAndroid,
  addMenuOpen,
  cloneCredentialId,
  settings.prepareCloneCredentials,
  closeModal,
)
const {
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
} = provisioning

const mobile = computed(() => viewportMobile.value || nativeAndroid.value)
const shellClass = computed(() => ({
  dark: dark.value,
  mobile: mobile.value,
  'sidebar-visible': sidebarOpen.value,
}))
const quickOpenResults = computed(() => {
  const needle = quickOpenQuery.value.trim().toLowerCase()
  return workspace.quickOpenDocuments.value
    .filter((document) => !needle || document.path.toLowerCase().includes(needle))
    .slice(0, 40)
})

document.documentElement.dataset.theme = dark.value ? 'dark' : 'light'

onMounted(() => {
  window.addEventListener('keydown', handleShortcut)
})

onBeforeUnmount(() => {
  window.removeEventListener('keydown', handleShortcut)
})

watch(dark, (value) => {
  localStorage.setItem('marktree-theme', value ? 'dark' : 'light')
  document.documentElement.dataset.theme = value ? 'dark' : 'light'
})

function handleShortcut(event: KeyboardEvent) {
  if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 'p') {
    event.preventDefault()
    quickOpenQuery.value = ''
    quickOpen.value = true
  }
  if (event.key === 'Escape') {
    quickOpen.value = false
    if (modal.value) closeModal()
  }
}

async function chooseQuickOpen(path: string) {
  quickOpen.value = false
  await workspace.openDocument(path)
}

function abortPendingOperation() {
  if (window.confirm(t('app.abortGitOperationConfirm'))) {
    void workspace.abortGitOperation()
  }
}
</script>

<template>
  <div class="app-shell" :class="shellClass">
    <RepositoryRail
      v-if="!mobile"
      :repositories="workspace.repositories.value"
      :active-id="workspace.activeRepositoryId.value"
      :dark="dark"
      @select="workspace.activateRepository"
      @add="addMenuOpen = !addMenuOpen"
      @toggle-theme="dark = !dark"
    />

    <header v-if="mobile" class="mobile-topbar">
      <button @click="sidebarOpen = !sidebarOpen"><Menu :size="21" /></button>
      <div>
        <select
          v-if="workspace.repositories.value.length"
          :value="workspace.activeRepositoryId.value"
          :aria-label="$t('app.repository')"
          @change="workspace.activateRepository(($event.target as HTMLSelectElement).value)"
        >
          <option
            v-for="repository in workspace.repositories.value"
            :key="repository.id"
            :value="repository.id"
          >
            {{ repository.name }}
          </option>
        </select>
        <strong v-else>Marktree</strong>
        <span class="mobile-subtitle">
          {{ workspace.activeWorktree.value?.branch ?? $t('app.subtitle') }}
          ·
          <button
            v-if="workspace.pendingOperation.value"
            :disabled="workspace.syncing.value"
            @click="abortPendingOperation"
          >
            {{ $t('app.abortGitOperation') }}
          </button>
          <template v-if="workspace.pendingOperation.value"> · </template>
          <button :disabled="workspace.syncing.value" @click="workspace.sync">
            {{ workspace.syncing.value ? $t('app.syncing') : $t('app.sync') }}
          </button>
        </span>
      </div>
    </header>

    <div v-if="addMenuOpen" class="add-menu">
      <button @click="chooseRepository('open')"><FolderGit2 :size="16" /> {{ $t('app.open') }}</button>
      <button @click="openCloneDialog"><Cloud :size="16" /> {{ $t('app.clone') }}</button>
      <button @click="chooseRepository('initialize')"><Plus :size="16" /> {{ $t('app.initialize') }}</button>
    </div>

    <template v-if="workspace.activeRepository.value">
      <WorkspaceSidebar
        :repository="workspace.activeRepository.value"
        :active-worktree="workspace.activeWorktree.value"
        :documents="workspace.filteredDocuments.value"
        :search-query="workspace.searchQuery.value"
        :search-results="workspace.crossWorktreeMatches.value"
        :mobile="mobile"
        @update:search-query="workspace.searchQuery.value = $event"
        @search="workspace.search"
        @select-worktree="workspace.selectWorktree"
        @open-file="workspace.openDocument"
        @new-file="openNewDocument"
        @new-worktree="openWorktreeDialog"
        @open-window="openWorktreeWindow"
        @add-repository="openCloneDialog"
        @open-search-result="workspace.openSearchResult"
      />

      <section class="main-column">
        <header v-if="!mobile" class="workspace-topbar">
          <div class="branch-status">
            <strong>{{ workspace.activeWorktree.value?.name }}</strong>
            <span>{{ workspace.activeStatus.value?.branch ?? $t('app.detachedHead') }}</span>
            <i v-if="workspace.activeStatus.value?.ahead">
              ↑ {{ workspace.activeStatus.value.ahead }}
            </i>
            <i v-if="workspace.activeStatus.value?.behind">
              ↓ {{ workspace.activeStatus.value.behind }}
            </i>
          </div>
          <div class="topbar-actions">
            <select
              v-if="
                workspace.activeTab.value &&
                workspace.activeRepository.value.worktrees.length > 1
              "
              class="worktree-compare-select"
              value=""
              @change="
                workspace.showWorktreeDiff(($event.target as HTMLSelectElement).value);
                ($event.target as HTMLSelectElement).value = ''
              "
            >
              <option value="" disabled>{{ $t('app.compareWorktrees') }}</option>
              <option
                v-for="worktree in workspace.activeRepository.value.worktrees.filter(
                  (item) => item.path !== workspace.activeRoot.value,
                )"
                :key="worktree.path"
                :value="worktree.path"
              >
                {{ worktree.name }}
              </option>
            </select>
            <button @click="workspace.refreshActive(false)">
              <RefreshCw :size="16" /> {{ $t('app.refresh') }}
            </button>
            <button
              :disabled="Boolean(workspace.pendingOperation.value)"
              @click="gitPanelOpen = !gitPanelOpen"
            >
              <GitCompareArrows :size="16" />
              {{
                workspace.activeStatus.value?.changedCount
                  ? $t('app.changed', { count: workspace.activeStatus.value.changedCount })
                  : $t('app.clean')
              }}
            </button>
            <button
              v-if="workspace.pendingOperation.value"
              :disabled="workspace.syncing.value"
              @click="abortPendingOperation"
            >
              <X :size="16" /> {{ $t('app.abortGitOperation') }}
            </button>
            <button
              class="primary"
              :disabled="workspace.syncing.value"
              @click="workspace.sync"
            >
              <RefreshCw :size="16" :class="{ spinning: workspace.syncing.value }" />
              {{ workspace.syncing.value ? $t('app.syncing') : $t('app.sync') }}
            </button>
            <button class="icon-only" @click="openCredentials"><MoreHorizontal :size="18" /></button>
          </div>
        </header>

        <EditorWorkspace
          :tabs="workspace.tabs.value"
          :active-tab="workspace.activeTab.value"
          :active-key="workspace.activeTabKey.value"
          :dark="dark"
          :load-repository-image="workspace.loadRepositoryImage"
          @activate="workspace.activeTabKey.value = $event"
          @close="workspace.closeTab"
          @update-content="workspace.updateActiveContent"
          @asset="workspace.writeImage"
          @diff="
            workspace.activeTab.value?.dirty
              ? workspace.showUnsavedDiff()
              : workspace.showDiff('worktreeToHead')
          "
        />

        <footer class="status-bar">
          <span>{{ workspace.activeRoot.value }}</span>
          <span>
            {{ workspace.activeTab.value?.encoding ?? 'utf8' }}
            · {{ workspace.activeTab.value?.lineEnding ?? 'none' }}
            ·
            {{ workspace.activeStatus.value?.upstream ?? $t('app.remote') }}
            · {{ workspace.activeStatus.value?.branch ?? 'HEAD' }}
          </span>
        </footer>
      </section>

      <GitPanel
        v-if="
          gitPanelOpen &&
          !mobile &&
          workspace.activeStatus.value &&
          !workspace.pendingOperation.value
        "
        :status="workspace.activeStatus.value"
        :branches="workspace.branches.value"
        @close="gitPanelOpen = false"
        @action="workspace.gitAction"
        @set-staged="workspace.setPathStaged"
        @create-branch="workspace.createBranch"
        @checkout-branch="workspace.checkoutBranch"
        @delete-branch="workspace.deleteBranch"
      />
      <DiffPanel
        v-if="workspace.diffOpen.value && workspace.diffResult.value"
        :result="workspace.diffResult.value"
        @close="workspace.diffOpen.value = false"
        @mode="workspace.showDiff"
      />
    </template>

    <main v-else class="welcome">
      <div class="welcome-copy">
        <img src="/marktree.svg" alt="" />
        <span>MARKTREE</span>
        <h1>{{ $t('app.welcomeTitle') }}</h1>
        <p>{{ $t('app.welcomeBody') }}</p>
        <div class="welcome-actions">
          <button v-if="!nativeAndroid" class="primary" @click="chooseRepository('open')">
            <FolderGit2 :size="18" /> {{ $t('app.open') }}
          </button>
          <button :class="{ primary: nativeAndroid }" @click="openCloneDialog">
            <Cloud :size="18" /> {{ $t('app.clone') }}
          </button>
          <button @click="chooseRepository('initialize')">
            <Plus :size="18" /> {{ $t('app.initialize') }}
          </button>
        </div>
      </div>
    </main>

    <div
      v-if="workspace.message.value || workspace.error.value"
      class="notice"
      :class="{ error: workspace.error.value }"
    >
      <span>{{ workspace.error.value || workspace.message.value }}</span>
      <button @click="workspace.clearNotice"><X :size="15" /></button>
    </div>

    <RepositoryDialogs
      :modal="modal"
      :form="form"
      :native-android="nativeAndroid"
      :auth-configuration="authConfiguration"
      :github-device="githubDevice"
      :github-pending="githubPending"
      :clone-credential-id="cloneCredentialId"
      @close="closeModal"
      @choose-clone-destination="chooseCloneDestination"
      @clone="cloneRepository"
      @begin-github-login="beginGithubLogin"
      @create-document="createDocument"
      @initialize-mobile-repository="initializeMobileRepository"
      @create-worktree="createWorktree"
      @save-generic-credential="saveGenericCredential"
      @save-repository-config="saveRepositoryConfig"
      @forget-active-repository="forgetActiveRepository"
    />

    <WorkspaceOverlays
      :external-comparison="workspace.externalComparison.value"
      :quick-open="quickOpen"
      :quick-open-query="quickOpenQuery"
      :quick-open-results="quickOpenResults"
      :image-preview="workspace.imagePreview.value"
      @choose-external-version="workspace.chooseExternalVersion"
      @close-quick-open="quickOpen = false"
      @update-quick-open-query="quickOpenQuery = $event"
      @choose-quick-open="chooseQuickOpen"
      @close-image-preview="workspace.imagePreview.value = undefined"
    />

    <ConflictDialog
      v-if="workspace.conflicts.value.length"
      :conflicts="workspace.conflicts.value"
      :native-android="nativeAndroid"
      :syncing="workspace.syncing.value"
      @abort="abortPendingOperation"
      @resolve-choice="workspace.resolveConflict"
      @resolve-content="workspace.resolveConflictContent"
    />
  </div>
</template>
