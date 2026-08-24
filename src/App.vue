<script setup lang="ts">
import {
  Cloud,
  FolderOpen,
  FolderPlus,
  X,
} from 'lucide-vue-next'
import {
  computed,
  defineAsyncComponent,
  nextTick,
  onBeforeUnmount,
  onMounted,
  ref,
  watch,
} from 'vue'
import { useI18n } from 'vue-i18n'

import ConflictDialog from '@/components/ConflictDialog.vue'
import AndroidShareDialog from '@/components/AndroidShareDialog.vue'
import DiffPanel from '@/components/DiffPanel.vue'
import GitPanel from '@/components/GitPanel.vue'
import WorkspaceRail from '@/components/WorkspaceRail.vue'
import WorkspaceDialogs from '@/components/WorkspaceDialogs.vue'
import WindowTitlebar from '@/components/WindowTitlebar.vue'
import WorkspaceSidebar, {
  type WorkspaceEntryActionRequest,
} from '@/components/WorkspaceSidebar.vue'
import WorkspaceOverlays from '@/components/WorkspaceOverlays.vue'
import WorkspaceTopbar from '@/components/WorkspaceTopbar.vue'
import {
  useDialogState,
  type AppModal,
  type ConfirmationDialogState,
} from '@/composables/app/dialogState'
import { useAndroidShare } from '@/composables/app/useAndroidShare'
import { useCommandPalette } from '@/composables/app/useCommandPalette'
import { useNativeSmoke } from '@/composables/app/useNativeSmoke'
import { useWorkspaceProvisioning } from '@/composables/app/useWorkspaceProvisioning'
import { useWorkspaceSettings } from '@/composables/app/useWorkspaceSettings'
import { useWorkspaceLifecycle } from '@/composables/app/useWorkspaceLifecycle'
import { useWorkspace } from '@/composables/useWorkspace'
import { editorFontStack, editorPreferences } from '@/lib/editor/preferences'
import type { WorktreeSearchResult } from '@/types'

const EditorWorkspace = defineAsyncComponent(
  () => import('@/components/EditorWorkspace.vue'),
)
const workspace = useWorkspace()
const { t } = useI18n()
const {
  modal,
  form,
  dialogBusy,
  dialogError,
  openDialog,
  closeDialog,
  runDialogAction,
} = useDialogState()

const dark = ref(localStorage.getItem('marktree-theme') === 'dark')
const sidebarOpen = ref(false)
const gitPanelOpen = ref(false)
const addMenuOpen = ref(false)
const addMenu = ref<HTMLElement>()
const confirmation = ref<ConfirmationDialogState>()
let confirmationAction: (() => Promise<boolean>) | undefined
let confirmationReturnModal: AppModal | undefined
const editorWorkspace = ref<{
  focusAtLine: (line: number, column?: number) => void
  insertText: (text: string) => void
  openSnippetManager: () => void
}>()
const workspaceSidebar = ref<{ revealPath: (path: string) => void }>()

const lifecycle = useWorkspaceLifecycle(workspace)
const {
  viewportMobile,
  nativeAndroid,
  ready,
  platformPulse,
  clearWorkspaceTimers,
} = lifecycle
useNativeSmoke(workspace, ready)

const settings = useWorkspaceSettings(
  workspace,
  modal,
  form,
  clearWorkspaceTimers,
  openDialog,
  runDialogAction,
  closeDialog,
)
const {
  authConfiguration,
  githubDevice,
  githubPending,
  cloneCredentialId,
  operationLog,
  openSettings,
  saveGenericCredential,
  saveWorkspaceConfig,
  forgetActiveWorkspace,
  beginGithubLogin,
  closeModal,
} = settings

const provisioning = useWorkspaceProvisioning(
  workspace,
  form,
  nativeAndroid,
  addMenuOpen,
  cloneCredentialId,
  settings.prepareCloneCredentials,
  openDialog,
  runDialogAction,
  closeModal,
)
const {
  chooseWorkspace,
  chooseCloneDestination,
  openCloneDialog,
  cloneGitWorkspace,
  createMobileWorkspace,
  createDesktopWorkspace,
  openNewDocument,
  createDocument,
  openWorktreeDialog,
  createWorktree,
  openWorktreeWindow,
} = provisioning

const androidShare = useAndroidShare(
  workspace,
  nativeAndroid,
  (markdown) => editorWorkspace.value?.insertText(markdown),
)

watch(workspace.activeRoot, () => {
  sidebarOpen.value = false
  if (modal.value && !['clone', 'mobileWorkspace'].includes(modal.value)) {
    closeModal()
  }
})
watch(modal, (value) => {
  if (value === 'confirmation') return
  confirmation.value = undefined
  confirmationAction = undefined
})
watch(platformPulse, () => void androidShare.detect(), { immediate: true })

const commandPalette = useCommandPalette(workspace, {
  nativeAndroid,
  async executeAction(action) {
    if (action === 'addWorkspace') {
      await chooseWorkspace(nativeAndroid.value ? 'create' : 'open')
    } else if (action === 'newDocument') openNewDocument()
    else if (action === 'newFolder') openEntryAction({ action: 'newFolder', directory: '' })
    else if (action === 'refresh') await workspace.refreshActive(false)
    else if (action === 'settings') openSettings()
    else if (action === 'sync') await workspace.sync()
    else if (action === 'snippets') editorWorkspace.value?.openSnippetManager()
  },
  closeModal,
  focusResult: (line, column) => editorWorkspace.value?.focusAtLine(line, column),
})

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
  compact: editorPreferences.density === 'compact',
}))
const shellStyle = computed(() => ({
  '--sidebar-width': `${editorPreferences.sidebarWidth}px`,
  '--editor-font-size': `${editorPreferences.fontSize}px`,
  '--editor-font-family': editorFontStack(editorPreferences.fontFamily),
}))
document.documentElement.dataset.theme = dark.value ? 'dark' : 'light'

function dismissAddMenu(event: PointerEvent) {
  if (!addMenuOpen.value) return
  const target = event.target as Element
  if (
    addMenu.value?.contains(target) ||
    target.closest('.workspace-button.subtle, .sidebar-add-workspace')
  ) return
  addMenuOpen.value = false
}

function closeAddMenuOnEscape(event: KeyboardEvent) {
  if (event.key === 'Escape') addMenuOpen.value = false
}

onMounted(() => {
  document.addEventListener('pointerdown', dismissAddMenu, true)
  window.addEventListener('keydown', closeAddMenuOnEscape)
})

onBeforeUnmount(() => {
  document.removeEventListener('pointerdown', dismissAddMenu, true)
  window.removeEventListener('keydown', closeAddMenuOnEscape)
})

watch(dark, (value) => {
  localStorage.setItem('marktree-theme', value ? 'dark' : 'light')
  document.documentElement.dataset.theme = value ? 'dark' : 'light'
})

async function revealWorkspacePath(path: string) {
  if (mobile.value) sidebarOpen.value = true
  await nextTick()
  workspaceSidebar.value?.revealPath(path)
}

async function openSidebarSearchResult(result: WorktreeSearchResult) {
  await workspace.openSearchResult(result)
  await nextTick()
  if (result.line) editorWorkspace.value?.focusAtLine(result.line, result.column ?? 1)
  if (mobile.value) sidebarOpen.value = false
}

function openEntryAction(request: WorkspaceEntryActionRequest) {
  form.entryAction = request.action
  form.entryDirectory = request.directory
  form.entrySourcePath = request.sourcePath ?? ''
  form.entryOriginalName = request.entryName ?? ''
  form.entryName = request.suggestedName ?? request.entryName ?? ''
  openDialog('workspaceEntry')
}

async function submitEntryAction() {
  const name = form.entryName.trim()
  const destination = form.entryDirectory
    ? `${form.entryDirectory}/${name}`
    : name
  const completed = await runDialogAction(async () => {
    let succeeded = false
    if (form.entryAction === 'newFolder') {
      succeeded = Boolean(name) && await workspace.createFolder(destination)
    } else if (form.entryAction === 'rename') {
      succeeded = Boolean(name) && name !== form.entryOriginalName &&
        await workspace.moveWorkspaceEntry(form.entrySourcePath, destination)
    } else if (form.entryAction === 'duplicate') {
      succeeded = Boolean(name) &&
        await workspace.duplicateWorkspaceEntry(form.entrySourcePath, destination)
    } else {
      succeeded = await workspace.trashWorkspaceEntry(form.entrySourcePath)
    }
    if (!succeeded) throw new Error(workspace.error.value || t('app.errorOperationFailed'))
    return true
  })
  if (completed) closeModal()
}

function requestConfirmation(
  state: ConfirmationDialogState,
  action: () => Promise<boolean>,
) {
  confirmationReturnModal = modal.value
  confirmation.value = state
  confirmationAction = action
  openDialog('confirmation')
}

function closeCurrentModal() {
  if (modal.value === 'confirmation' && confirmationReturnModal) {
    const returnTo = confirmationReturnModal
    confirmationReturnModal = undefined
    openDialog(returnTo)
    return
  }
  confirmationReturnModal = undefined
  closeModal()
}

async function confirmPendingAction() {
  const action = confirmationAction
  if (!action) return
  const completed = await runDialogAction(async () => {
    if (!await action()) {
      throw new Error(workspace.error.value || t('app.errorOperationFailed'))
    }
    return true
  })
  if (completed) {
    confirmationReturnModal = undefined
    closeModal()
  }
}

function abortPendingOperation() {
  requestConfirmation(
    {
      title: t('app.abortGitOperation'),
      message: t('app.abortGitOperationConfirm'),
      confirmLabel: t('app.abortGitOperation'),
      danger: true,
    },
    workspace.abortGitOperation,
  )
}

function requestDeleteBranch(name: string): Promise<boolean> {
  requestConfirmation(
    {
      title: t('app.deleteBranch'),
      message: t('app.deleteBranchConfirm', { name }),
      confirmLabel: t('app.deleteBranch'),
      danger: true,
    },
    () => workspace.deleteBranch(name),
  )
  return Promise.resolve(false)
}

function requestForgetWorkspace() {
  requestConfirmation(
    {
      title: t('app.forgetWorkspace'),
      message: t('app.forgetWorkspaceConfirm'),
      confirmLabel: t('app.forgetWorkspace'),
      danger: true,
    },
    forgetActiveWorkspace,
  )
}

function requestEmptyTrash() {
  requestConfirmation(
    {
      title: t('app.emptyTrash'),
      message: t('app.emptyTrashConfirm'),
      confirmLabel: t('app.emptyTrash'),
      danger: true,
    },
    workspace.emptyWorkspaceTrash,
  )
}

async function requestEnableGit() {
  const message = await runDialogAction(workspace.previewWorkspaceGitBaseline)
  if (!message) return
  requestConfirmation(
    {
      title: t('app.enableGit'),
      message,
      confirmLabel: t('app.enableGit'),
    },
    workspace.enableWorkspaceGit,
  )
}
</script>

<template>
  <div class="app-frame" :class="{ mobile }">
    <WindowTitlebar
      v-if="!mobile"
      :context="titlebarContext"
      @error="workspace.reportError($event)"
    />
    <div class="app-shell" :class="shellClass" :style="shellStyle">
    <WorkspaceRail
      v-if="!mobile"
      :workspaces="workspace.workspaces.value"
      :active-id="workspace.activeWorkspaceId.value"
      :dark="dark"
      @select="workspace.activateWorkspace"
      @add="addMenuOpen = !addMenuOpen"
      @toggle-theme="dark = !dark"
    />

    <WorkspaceTopbar
      v-if="mobile"
      :mobile="mobile"
      :workspaces="workspace.workspaces.value"
      :active-workspace-id="workspace.activeWorkspaceId.value"
      :active-workspace="workspace.activeWorkspace.value"
      :active-worktree="workspace.activeWorktree.value"
      :active-status="workspace.activeStatus.value"
      :pending-operation="workspace.pendingOperation.value"
      :syncing="workspace.syncing.value"
      :loading="workspace.loading.value"
      @toggle-sidebar="sidebarOpen = !sidebarOpen"
      @activate-workspace="workspace.activateWorkspace"
      @command-palette="commandPalette.show()"
      @settings="openSettings"
      @refresh="workspace.refreshActive(false)"
      @toggle-git-panel="gitPanelOpen = !gitPanelOpen"
      @abort="abortPendingOperation"
      @sync="workspace.sync"
    />

    <div v-if="addMenuOpen" ref="addMenu" class="add-menu">
      <button @click="chooseWorkspace('open')"><FolderOpen :size="16" /> {{ $t('app.openFolder') }}</button>
      <button @click="chooseWorkspace('create')"><FolderPlus :size="16" /> {{ $t('app.newFolder') }}</button>
      <button @click="openCloneDialog"><Cloud :size="16" /> {{ $t('app.clone') }}</button>
    </div>

    <template v-if="workspace.activeWorkspace.value">
      <WorkspaceSidebar
        ref="workspaceSidebar"
        :workspace="workspace.activeWorkspace.value"
        :entries="workspace.entries.value"
        :favorites="workspace.favoriteDocuments.value"
        :search-query="workspace.searchQuery.value"
        :search-results="workspace.searchResults.value"
        :searching="workspace.searchInProgress.value"
        :active-path="workspace.activeTab.value?.path"
        :mobile="mobile"
        @update:search-query="workspace.setSearchQuery($event)"
        @search="workspace.search"
        @open-search-result="openSidebarSearchResult"
        @open-file="workspace.openDocument"
        @new-file="openNewDocument"
        @request-entry-action="openEntryAction"
        @move-entry="workspace.moveWorkspaceEntry"
        @open-system="workspace.openWithSystem"
        @add-workspace="addMenuOpen = !addMenuOpen"
        @toggle-favorite="workspace.toggleFavoritePath"
      />

      <section class="main-column">
        <WorkspaceTopbar
          v-if="!mobile"
          :mobile="false"
          :workspaces="workspace.workspaces.value"
          :active-workspace-id="workspace.activeWorkspaceId.value"
          :active-workspace="workspace.activeWorkspace.value"
          :active-worktree="workspace.activeWorktree.value"
          :active-status="workspace.activeStatus.value"
          :pending-operation="workspace.pendingOperation.value"
          :syncing="workspace.syncing.value"
          :loading="workspace.loading.value"
          @activate-workspace="workspace.activateWorkspace"
          @command-palette="commandPalette.show()"
          @settings="openSettings"
          @refresh="workspace.refreshActive(false)"
          @toggle-git-panel="gitPanelOpen = !gitPanelOpen"
          @abort="abortPendingOperation"
          @sync="workspace.sync"
        />
        <EditorWorkspace
          ref="editorWorkspace"
          :tabs="workspace.tabs.value"
          :entries="workspace.entries.value"
          :active-tab="workspace.activeTab.value"
          :active-key="workspace.activeTabKey.value"
          :dark="dark"
          :favorite="workspace.activeDocumentIsFavorite()"
          :load-workspace-image="workspace.loadWorkspaceImage"
          :document-character-limit="workspace.activeDocumentCharacterLimit.value"
          :can-compare="Boolean(workspace.activeWorkspace.value.git)"
          @activate="workspace.selectTab($event)"
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
          @open-path="workspace.openDocument"
          @toggle-favorite="workspace.toggleActiveDocumentFavorite"
          @reveal-path="revealWorkspacePath"
          @error="workspace.reportError($event)"
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
        :busy-action="workspace.gitBusyAction.value"
        :run-action="workspace.gitAction"
        :set-staged="workspace.setPathStaged"
        :create-branch="workspace.createBranch"
        :checkout-branch="workspace.checkoutBranch"
        :delete-branch="requestDeleteBranch"
        @close="gitPanelOpen = false"
        @select-worktree="workspace.selectWorktree"
        @new-worktree="openWorktreeDialog"
        @open-window="openWorktreeWindow"
        @settings="openSettings"
      />
      <DiffPanel
        v-if="workspace.diffOpen.value && workspace.diffResult.value"
        :result="workspace.diffResult.value"
        @close="workspace.closeDiff"
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
    <div v-if="workspace.loading.value" class="workspace-progress" role="status">
      <span />
      <em>{{ $t('app.working') }}</em>
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
      :operation-log="operationLog"
      :busy="dialogBusy"
      :error="dialogError"
      :confirmation="confirmation"
      @close="closeCurrentModal"
      @choose-clone-destination="chooseCloneDestination"
      @clone="cloneGitWorkspace"
      @begin-github-login="beginGithubLogin"
      @create-document="createDocument"
      @create-mobile-workspace="createMobileWorkspace"
      @create-desktop-workspace="createDesktopWorkspace"
      @create-worktree="createWorktree"
      @submit-entry-action="submitEntryAction"
      @save-generic-credential="saveGenericCredential"
      @save-workspace-config="saveWorkspaceConfig"
      @enable-git="requestEnableGit"
      @forget-active-workspace="requestForgetWorkspace"
      @restore-trash="workspace.restoreWorkspaceTrash"
      @empty-trash="requestEmptyTrash"
      @confirm="confirmPendingAction"
      @export-workspace-archive="androidShare.exportWorkspace"
    />

    <AndroidShareDialog
      v-if="androidShare.pending.value"
      :share="androidShare.pending.value"
      :workspaces="workspace.workspaces.value"
      :selected-root="androidShare.selectedRoot.value"
      :selected-directory="androidShare.selectedDirectory.value"
      :directories="androidShare.directories.value"
      :active-root="workspace.activeRoot.value"
      :active-document-path="
        workspace.activeTab.value && /\.(?:md|markdown|mdx)$/i.test(workspace.activeTab.value.path)
          ? workspace.activeTab.value.path
          : undefined
      "
      :importing="androidShare.importing.value"
      @close="androidShare.pending.value = undefined"
      @select-root="androidShare.selectRoot"
      @select-directory="androidShare.selectedDirectory.value = $event"
      @import="androidShare.importShare"
    />

    <WorkspaceOverlays
      :external-comparison="workspace.externalComparison.value"
      :command-palette-open="commandPalette.open.value"
      :command-palette-query="commandPalette.query.value"
      :command-palette-results="commandPalette.results.value"
      :command-palette-searching="commandPalette.searching.value"
      :command-palette-path-prefix="commandPalette.pathPrefix.value"
      :command-palette-file-kind="commandPalette.fileKind.value"
      :command-palette-modified-days="commandPalette.modifiedDays.value"
      :file-preview="workspace.filePreview.value"
      @choose-external-version="workspace.chooseExternalVersion"
      @close-command-palette="commandPalette.open.value = false"
      @update-command-palette-query="commandPalette.query.value = $event"
      @update-command-palette-path-prefix="commandPalette.pathPrefix.value = $event"
      @update-command-palette-file-kind="commandPalette.fileKind.value = $event"
      @update-command-palette-modified-days="commandPalette.modifiedDays.value = $event"
      @choose-command-palette="commandPalette.choose"
      @close-file-preview="workspace.closeFilePreview"
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
