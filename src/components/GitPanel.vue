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

import type { BranchDescriptor, GitStatusSnapshot, WorktreeDescriptor } from '@/types'

const props = defineProps<{
  status: GitStatusSnapshot
  branches: BranchDescriptor[]
  worktrees: WorktreeDescriptor[]
  activeRoot?: string
  busyAction?: string
  runAction: (
    action: 'fetch' | 'pull' | 'push' | 'stageAll' | 'commit',
    payload?: string,
  ) => Promise<boolean>
  setStaged: (path: string, staged: boolean) => Promise<boolean>
  createBranch: (name: string, startPoint?: string) => Promise<boolean>
  checkoutBranch: (name: string) => Promise<boolean>
  deleteBranch: (name: string) => Promise<boolean>
}>()

const emit = defineEmits<{
  close: []
  selectWorktree: [worktree: WorktreeDescriptor]
  newWorktree: []
  openWindow: [worktree: WorktreeDescriptor]
  settings: []
}>()

const commitMessage = ref('')
const branchName = ref('')
const branchStart = ref('')
const busy = computed(() => Boolean(props.busyAction))
const staged = computed(() => props.status.files.filter((file) => file.staged))
const unstaged = computed(() =>
  props.status.files.filter((file) => file.worktreeStatus !== 'clean' && !file.conflicted),
)

async function submitBranch() {
  const name = branchName.value.trim()
  if (!name || busy.value) return
  if (await props.createBranch(name, branchStart.value.trim() || undefined)) {
    branchName.value = ''
    branchStart.value = ''
  }
}

async function switchBranch(event: Event) {
  const name = (event.target as HTMLSelectElement).value
  if (name && name !== props.status.branch && !busy.value) {
    await props.checkoutBranch(name)
  }
}

async function requestDelete(branch: BranchDescriptor) {
  if (!branch.checkedOutPath) await props.deleteBranch(branch.name)
}

async function submitCommit() {
  const message = commitMessage.value.trim()
  if (!message || !staged.value.length || busy.value) return
  if (await props.runAction('commit', message)) commitMessage.value = ''
}
</script>

<template>
  <aside class="git-panel">
    <header class="panel-header">
      <div>
        <h3>{{ $t('app.advanced') }}</h3>
        <span>{{ status.branch ?? $t('app.detachedHead') }}</span>
      </div>
      <button :aria-label="$t('app.close')" :disabled="busy" @click="$emit('close')">×</button>
    </header>
    <div v-if="busy" class="git-operation-state" role="status">
      <RefreshCw :size="14" /> {{ $t('app.working') }}
    </div>
    <div class="git-summary">
      <span><ArrowUpFromLine :size="14" /> {{ $t('app.ahead', { count: status.ahead }) }}</span>
      <span><ArrowDownToLine :size="14" /> {{ $t('app.behind', { count: status.behind }) }}</span>
    </div>
    <section class="branch-manager advanced-worktrees">
      <header class="section-title">
        <span><Box :size="14" /> {{ $t('app.worktrees') }}</span>
        <button :aria-label="$t('app.newWorktree')" :disabled="busy" @click="$emit('newWorktree')"><Plus :size="14" /></button>
      </header>
      <div
        v-for="worktree in worktrees"
        :key="worktree.path"
        class="branch-row worktree-row"
        :class="{ active: worktree.path === activeRoot }"
        @click="!busy && $emit('selectWorktree', worktree)"
      >
        <span>
          <strong>{{ worktree.name }}</strong>
          <small>{{ worktree.branch ?? $t('app.detachedHead') }}</small>
        </span>
        <button :aria-label="$t('app.openWorktreeWindow')" :disabled="busy" @click.stop="$emit('openWindow', worktree)">
          <SquareArrowOutUpRight :size="13" />
        </button>
      </div>
      <button class="git-settings-button" :disabled="busy" @click="$emit('settings')">
        <Settings :size="14" /> {{ $t('app.gitSettings') }}
      </button>
    </section>
    <section class="branch-manager">
      <label>
        <span><GitBranch :size="14" /> {{ $t('app.branch') }}</span>
        <select :value="status.branch" :disabled="busy" @change="switchBranch">
          <option v-for="branch in branches" :key="branch.name" :value="branch.name">
            {{ branch.name }}
          </option>
        </select>
      </label>
      <form @submit.prevent="submitBranch">
        <input v-model="branchName" :disabled="busy" :placeholder="$t('app.newBranch')" />
        <input v-model="branchStart" :disabled="busy" :placeholder="$t('app.startPoint')" />
        <button :title="$t('app.createBranch')" :aria-label="$t('app.createBranch')" :disabled="busy || !branchName.trim()">
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
            :disabled="busy || Boolean(branch.checkedOutPath)"
            :title="
              branch.checkedOutPath
                ? $t('app.branchCheckedOut', { path: branch.checkedOutPath })
                : $t('app.deleteBranch')
            "
            :aria-label="$t('app.deleteBranch')"
            @click="requestDelete(branch)"
          >
            <Trash2 :size="13" />
          </button>
        </div>
      </div>
    </section>
    <div class="git-toolbar">
      <button :disabled="busy" @click="runAction('fetch')"><RefreshCw :size="15" /> {{ $t('app.fetch') }}</button>
      <button :disabled="busy" @click="runAction('pull')"><ArrowDownToLine :size="15" /> {{ $t('app.pull') }}</button>
      <button :disabled="busy" @click="runAction('push')"><ArrowUpFromLine :size="15" /> {{ $t('app.push') }}</button>
    </div>

    <section class="change-group">
      <header>
        <span>{{ $t('app.staged') }} · {{ staged.length }}</span>
      </header>
      <button
        v-for="file in staged"
        :key="`staged-${file.path}`"
        class="change-row"
        :disabled="busy"
        @click="setStaged(file.path, false)"
      >
        <Check :size="14" />
        <span>{{ file.path }}</span>
        <i>{{ file.indexStatus.slice(0, 1).toUpperCase() }}</i>
      </button>
    </section>

    <section class="change-group">
      <header>
        <span>{{ $t('app.unstaged') }} · {{ unstaged.length }}</span>
        <button v-if="unstaged.length" :aria-label="$t('app.stageAll')" :disabled="busy" @click="runAction('stageAll')">+</button>
      </header>
      <button
        v-for="file in unstaged"
        :key="`unstaged-${file.path}`"
        class="change-row"
        :disabled="busy"
        @click="setStaged(file.path, true)"
      >
        <CircleDot :size="14" />
        <span>{{ file.path }}</span>
        <i>{{ file.worktreeStatus.slice(0, 1).toUpperCase() }}</i>
      </button>
    </section>

    <form
      class="commit-box"
      @submit.prevent="submitCommit"
    >
      <textarea v-model="commitMessage" :disabled="busy" :placeholder="$t('app.commitPlaceholder')" />
      <button :disabled="busy || !commitMessage.trim() || !staged.length">
        <GitCommitHorizontal :size="16" /> {{ $t('app.commit') }}
      </button>
    </form>
  </aside>
</template>
