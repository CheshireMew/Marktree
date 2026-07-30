<script setup lang="ts">
import {
  Cloud,
  FolderOpen,
  FolderPlus,
  GitBranch,
  Menu,
  MoreHorizontal,
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
import WorkspaceRail from '@/components/WorkspaceRail.vue'
import WorkspaceDialogs from '@/components/WorkspaceDialogs.vue'
import WindowTitlebar from '@/components/WindowTitlebar.vue'
import WorkspaceSidebar from '@/components/WorkspaceSidebar.vue'
import WorkspaceOverlays from '@/components/WorkspaceOverlays.vue'
import { useDialogState } from '@/composables/app/dialogState'
import { useWorkspaceProvisioning } from '@/composables/app/useWorkspaceProvisioning'
import { useWorkspaceSettings } from '@/composables/app/useWorkspaceSettings'
import { useWorkspaceLifecycle } from '@/composables/app/useWorkspaceLifecycle'
import { useWorkspace } from '@/composables/useWorkspace'
import { readableError } from '@/lib/errors'

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
  if (modal.value && !['clone', 'mobileWorkspace'].includes(modal.value)) {
    closeActiveModal()
  }
})
const { viewportMobile, nativeAndroid, clearWorkspaceTimers } = lifecycle

const settings = useWorkspaceSettings(
  workspace,
  modal,
  form,
  clearWorkspaceTimers,
)
closeActiveModal = settings.closeModal
const {
  authConfiguration,
  githubDevice,
  githubPending,
  cloneCredentialId,
  openSettings,
  saveGenericCredential,
  saveWorkspaceConfig,
  forgetActiveWorkspace,
  beginGithubLogin,
  closeModal,
} = settings

const provisioning = useWorkspaceProvisioning(
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
} = provisioning

const mobile = computed(() => viewportMobile.value || nativeAndroid.value)
const titlebarContext = computed(() => {
  const activeWorkspace = workspace.activeWorkspace.value
  const worktree = workspace.activeWorktree.value
  if (!activeWorkspace) return undefined
  return worktree
    ? `${activeWorkspace.name} · ${worktree.name}`
    : activeWorkspace.name
})
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
  <div class="app-frame" :class="{ mobile }">
    <WindowTitlebar
      v-if="!mobile"
      :context="titlebarContext"
      @error="workspace.error.value = readableError($event)"
    />
    <div class="app-shell" :class="shellClass">
    <WorkspaceRail
      v-if="!mobile"
      :workspaces="workspace.workspaces.value"
      :active-id="workspace.activeWorkspaceId.value"
      :dark="dark"
      @select="workspace.activateWorkspace"
      @add="addMenuOpen = !addMenuOpen"
      @toggle-theme="dark = !dark"
    />

    <header v-if="mobile" class="mobile-topbar">
      <button @click="sidebarOpen = !sidebarOpen"><Menu :size="21" /></button>
      <div>
        <select
          v-if="workspace.workspaces.value.length"
          :value="workspace.activeWorkspaceId.value"
          :aria-label="$t('app.workspace')"
          @change="workspace.activateWorkspace(($event.target as HTMLSelectElement).value)"
        >
          <option
            v-for="item in workspace.workspaces.value"
            :key="item.id"
            :value="item.id"
          >
            {{ item.name }}
          </option>
        </select>
        <strong v-else>Marktree</strong>
        <span class="mobile-subtitle">
          <template v-if="workspace.activeWorkspace.value?.git">
            {{ workspace.activeWorktree.value?.branch ?? $t('app.detachedHead') }}
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
          </template>
          <template v-else>{{ $t('app.localWorkspace') }}</template>
        </span>
      </div>
    </header>

    <div v-if="addMenuOpen" class="add-menu">
      <button @click="chooseWorkspace('open')"><FolderOpen :size="16" /> {{ $t('app.openFolder') }}</button>
      <button @click="chooseWorkspace('create')"><FolderPlus :size="16" /> {{ $t('app.newFolder') }}</button>
      <button @click="openCloneDialog"><Cloud :size="16" /> {{ $t('app.clone') }}</button>
    </div>

    <template v-if="workspace.activeWorkspace.value">
      <WorkspaceSidebar
        :workspace="workspace.activeWorkspace.value"
        :entries="workspace.filteredEntries.value"
        :search-query="workspace.searchQuery.value"
        :mobile="mobile"
        @update:search-query="workspace.searchQuery.value = $event"
        @search="workspace.search"
        @open-file="workspace.openDocument"
        @new-file="openNewDocument"
        @new-folder="workspace.createFolder"
        @move-entry="workspace.moveWorkspaceEntry"
        @trash-entry="workspace.trashWorkspaceEntry"
        @open-system="workspace.openWithSystem"
        @add-workspace="addMenuOpen = !addMenuOpen"
      />

      <section class="main-column">
        <header v-if="!mobile" class="workspace-topbar">
          <div class="branch-status">
            <strong>{{ workspace.activeWorkspace.value.name }}</strong>
            <template v-if="workspace.activeWorkspace.value.git">
              <span>{{ workspace.activeStatus.value?.branch ?? $t('app.detachedHead') }}</span>
              <i v-if="workspace.activeStatus.value?.ahead">
                ↑ {{ workspace.activeStatus.value.ahead }}
              </i>
              <i v-if="workspace.activeStatus.value?.behind">
                ↓ {{ workspace.activeStatus.value.behind }}
              </i>
            </template>
            <span v-else>{{ $t('app.localWorkspace') }}</span>
          </div>
          <div class="topbar-actions">
            <button @click="workspace.refreshActive(false)">
              <RefreshCw :size="16" /> {{ $t('app.refresh') }}
            </button>
            <button
              v-if="workspace.activeWorkspace.value.git"
              class="advanced-git-button"
              :disabled="Boolean(workspace.pendingOperation.value)"
              @click="gitPanelOpen = !gitPanelOpen"
            >
              <GitBranch :size="16" />
              {{
                workspace.activeStatus.value?.changedCount
                  ? $t('app.changed', { count: workspace.activeStatus.value.changedCount })
                  : $t('app.advancedGit')
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
              v-if="workspace.activeWorkspace.value.git"
              class="primary sync-button"
              :disabled="workspace.syncing.value"
              @click="workspace.sync"
            >
              <RefreshCw :size="16" :class="{ spinning: workspace.syncing.value }" />
              {{ workspace.syncing.value ? $t('app.syncing') : $t('app.sync') }}
            </button>
            <button
              v-else
              class="enable-git-button"
              @click="workspace.enableWorkspaceGit"
            >
              <GitBranch :size="16" /> {{ $t('app.enableGit') }}
            </button>
            <button class="icon-only" @click="openSettings"><MoreHorizontal :size="18" /></button>
          </div>
        </header>

        <EditorWorkspace
          :tabs="workspace.tabs.value"
          :active-tab="workspace.activeTab.value"
          :active-key="workspace.activeTabKey.value"
          :dark="dark"
          :load-workspace-image="workspace.loadWorkspaceImage"
          :can-compare="Boolean(workspace.activeWorkspace.value.git)"
          @activate="workspace.activeTabKey.value = $event"
          @close="workspace.closeTab"
          @update-content="workspace.updateActiveContent"
          @asset="workspace.writeImage"
          @diff="
            workspace.activeTab.value?.dirty
              ? workspace.showUnsavedDiff()
              : workspace.activeWorkspace.value.git
                ? workspace.showDiff('worktreeToHead')
                : undefined
          "
        />

        <footer class="status-bar">
          <span>{{ workspace.activeRoot.value }}</span>
          <span>
            {{ workspace.activeTab.value?.encoding ?? 'utf8' }}
            · {{ workspace.activeTab.value?.lineEnding ?? 'none' }}
            <template v-if="workspace.activeWorkspace.value.git">
              · {{ workspace.activeStatus.value?.upstream ?? $t('app.remote') }}
              · {{ workspace.activeStatus.value?.branch ?? 'HEAD' }}
            </template>
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
        :worktrees="workspace.activeWorkspace.value.git?.worktrees ?? []"
        :active-root="workspace.activeRoot.value"
        @close="gitPanelOpen = false"
        @action="workspace.gitAction"
        @set-staged="workspace.setPathStaged"
        @create-branch="workspace.createBranch"
        @checkout-branch="workspace.checkoutBranch"
        @delete-branch="workspace.deleteBranch"
        @select-worktree="workspace.selectWorktree"
        @new-worktree="openWorktreeDialog"
        @open-window="openWorktreeWindow"
        @settings="openSettings"
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
          <button v-if="!nativeAndroid" class="primary" @click="chooseWorkspace('open')">
            <FolderOpen :size="18" /> {{ $t('app.openFolder') }}
          </button>
          <button :class="{ primary: nativeAndroid }" @click="chooseWorkspace('create')">
            <FolderPlus :size="18" /> {{ $t('app.newFolder') }}
          </button>
          <button @click="openCloneDialog">
            <Cloud :size="18" /> {{ $t('app.cloneGitRepository') }}
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

    <WorkspaceDialogs
      :modal="modal"
      :form="form"
      :native-android="nativeAndroid"
      :git-enabled="Boolean(workspace.activeWorkspace.value?.git)"
      :auth-configuration="authConfiguration"
      :github-device="githubDevice"
      :github-pending="githubPending"
      :clone-credential-id="cloneCredentialId"
      :trash-entries="workspace.trashEntries.value"
      @close="closeModal"
      @choose-clone-destination="chooseCloneDestination"
      @clone="cloneGitWorkspace"
      @begin-github-login="beginGithubLogin"
      @create-document="createDocument"
      @create-mobile-workspace="createMobileWorkspace"
      @create-worktree="createWorktree"
      @save-generic-credential="saveGenericCredential"
      @save-workspace-config="saveWorkspaceConfig"
      @forget-active-workspace="forgetActiveWorkspace"
      @restore-trash="workspace.restoreWorkspaceTrash"
      @empty-trash="workspace.emptyWorkspaceTrash"
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
  </div>
</template>
