<script setup lang="ts">
import {
  ArrowDownToLine,
  ArrowUpFromLine,
  Box,
  Check,
  CircleDot,
  GitBranch,
  GitCommitHorizontal,
  Plus,
  RefreshCw,
  Settings,
  SquareArrowOutUpRight,
  Trash2,
} from 'lucide-vue-next'
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'

import type { BranchDescriptor, GitStatusSnapshot, WorktreeDescriptor } from '@/types'

const props = defineProps<{
  status: GitStatusSnapshot
  branches: BranchDescriptor[]
  worktrees: WorktreeDescriptor[]
  activeRoot?: string
}>()

const emit = defineEmits<{
  close: []
  action: [action: 'fetch' | 'pull' | 'push' | 'stageAll' | 'commit', payload?: string]
  setStaged: [path: string, staged: boolean]
  createBranch: [name: string, startPoint?: string]
  checkoutBranch: [name: string]
  deleteBranch: [name: string]
  selectWorktree: [worktree: WorktreeDescriptor]
  newWorktree: []
  openWindow: [worktree: WorktreeDescriptor]
  settings: []
}>()

const { t } = useI18n()
const commitMessage = ref('')
const branchName = ref('')
const branchStart = ref('')
const staged = computed(() => props.status.files.filter((file) => file.staged))
const unstaged = computed(() =>
  props.status.files.filter((file) => file.worktreeStatus !== 'clean' && !file.conflicted),
)

function submitBranch() {
  const name = branchName.value.trim()
  if (!name) return
  emit('createBranch', name, branchStart.value.trim() || undefined)
  branchName.value = ''
  branchStart.value = ''
}

function switchBranch(event: Event) {
  const name = (event.target as HTMLSelectElement).value
  if (name && name !== props.status.branch) emit('checkoutBranch', name)
}

function requestDelete(branch: BranchDescriptor) {
  if (
    branch.checkedOutPath ||
    !window.confirm(t('app.deleteBranchConfirm', { name: branch.name }))
  ) {
    return
  }
  emit('deleteBranch', branch.name)
}
</script>

<template>
  <aside class="git-panel">
    <header class="panel-header">
      <div>
        <h3>{{ $t('app.advanced') }}</h3>
        <span>{{ status.branch ?? $t('app.detachedHead') }}</span>
      </div>
      <button @click="$emit('close')">×</button>
    </header>
    <div class="git-summary">
      <span><ArrowUpFromLine :size="14" /> {{ $t('app.ahead', { count: status.ahead }) }}</span>
      <span><ArrowDownToLine :size="14" /> {{ $t('app.behind', { count: status.behind }) }}</span>
    </div>
    <section class="branch-manager advanced-worktrees">
      <header class="section-title">
        <span><Box :size="14" /> {{ $t('app.worktrees') }}</span>
        <button @click="$emit('newWorktree')"><Plus :size="14" /></button>
      </header>
      <div
        v-for="worktree in worktrees"
        :key="worktree.path"
        class="branch-row worktree-row"
        :class="{ active: worktree.path === activeRoot }"
        @click="$emit('selectWorktree', worktree)"
      >
        <span>
          <strong>{{ worktree.name }}</strong>
          <small>{{ worktree.branch ?? $t('app.detachedHead') }}</small>
        </span>
        <button @click.stop="$emit('openWindow', worktree)">
          <SquareArrowOutUpRight :size="13" />
        </button>
      </div>
      <button class="git-settings-button" @click="$emit('settings')">
        <Settings :size="14" /> {{ $t('app.gitSettings') }}
      </button>
    </section>
    <section class="branch-manager">
      <label>
        <span><GitBranch :size="14" /> {{ $t('app.branch') }}</span>
        <select :value="status.branch" @change="switchBranch">
          <option v-for="branch in branches" :key="branch.name" :value="branch.name">
            {{ branch.name }}
          </option>
        </select>
      </label>
      <form @submit.prevent="submitBranch">
        <input v-model="branchName" :placeholder="$t('app.newBranch')" />
        <input v-model="branchStart" :placeholder="$t('app.startPoint')" />
        <button :title="$t('app.createBranch')" :disabled="!branchName.trim()">
          <Plus :size="15" />
        </button>
      </form>
      <div class="branch-list">
        <div v-for="branch in branches" :key="`branch-${branch.name}`" class="branch-row">
          <span>
            <strong>{{ branch.name }}</strong>
            <small>
              {{ branch.upstream ?? $t('app.noUpstream') }}
              · ↑{{ branch.ahead }} ↓{{ branch.behind }}
            </small>
          </span>
          <button
            :disabled="Boolean(branch.checkedOutPath)"
            :title="
              branch.checkedOutPath
                ? $t('app.branchCheckedOut', { path: branch.checkedOutPath })
                : $t('app.deleteBranch')
            "
            @click="requestDelete(branch)"
          >
            <Trash2 :size="13" />
          </button>
        </div>
      </div>
    </section>
    <div class="git-toolbar">
      <button @click="$emit('action', 'fetch')"><RefreshCw :size="15" /> {{ $t('app.fetch') }}</button>
      <button @click="$emit('action', 'pull')"><ArrowDownToLine :size="15" /> {{ $t('app.pull') }}</button>
      <button @click="$emit('action', 'push')"><ArrowUpFromLine :size="15" /> {{ $t('app.push') }}</button>
    </div>

    <section class="change-group">
      <header>
        <span>{{ $t('app.staged') }} · {{ staged.length }}</span>
      </header>
      <button
        v-for="file in staged"
        :key="`staged-${file.path}`"
        class="change-row"
        @click="$emit('setStaged', file.path, false)"
      >
        <Check :size="14" />
        <span>{{ file.path }}</span>
        <i>{{ file.indexStatus.slice(0, 1).toUpperCase() }}</i>
      </button>
    </section>

    <section class="change-group">
      <header>
        <span>{{ $t('app.unstaged') }} · {{ unstaged.length }}</span>
        <button v-if="unstaged.length" @click="$emit('action', 'stageAll')">+</button>
      </header>
      <button
        v-for="file in unstaged"
        :key="`unstaged-${file.path}`"
        class="change-row"
        @click="$emit('setStaged', file.path, true)"
      >
        <CircleDot :size="14" />
        <span>{{ file.path }}</span>
        <i>{{ file.worktreeStatus.slice(0, 1).toUpperCase() }}</i>
      </button>
    </section>

    <form
      class="commit-box"
      @submit.prevent="
        $emit('action', 'commit', commitMessage);
        commitMessage = ''
      "
    >
      <textarea v-model="commitMessage" :placeholder="$t('app.commitPlaceholder')" />
      <button :disabled="!commitMessage.trim() || !staged.length">
        <GitCommitHorizontal :size="16" /> {{ $t('app.commit') }}
      </button>
    </form>
  </aside>
</template>
