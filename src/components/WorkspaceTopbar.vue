<script setup lang="ts">
import { GitBranch, Menu, MoreHorizontal, RefreshCw, Search } from 'lucide-vue-next'

import WorkspaceSyncControls from '@/components/WorkspaceSyncControls.vue'
import type {
  GitStatusSnapshot,
  PendingGitOperationSummary,
  WorkspaceDescriptor,
  WorktreeDescriptor,
} from '@/types'

defineProps<{
  mobile: boolean
  workspaces: WorkspaceDescriptor[]
  activeWorkspaceId?: string
  activeWorkspace?: WorkspaceDescriptor
  activeWorktree?: WorktreeDescriptor
  activeStatus?: GitStatusSnapshot
  pendingOperation?: PendingGitOperationSummary
  syncing: boolean
  loading?: boolean
}>()

defineEmits<{
  toggleSidebar: []
  activateWorkspace: [id: string]
  commandPalette: []
  settings: []
  refresh: []
  toggleGitPanel: []
  abort: []
  sync: []
}>()
</script>

<template>
  <header v-if="mobile" class="mobile-topbar">
    <button @click="$emit('toggleSidebar')"><Menu :size="21" /></button>
    <div>
      <select
        v-if="workspaces.length"
        :value="activeWorkspaceId"
        :aria-label="$t('app.workspace')"
        @change="$emit('activateWorkspace', ($event.target as HTMLSelectElement).value)"
      >
        <option v-for="item in workspaces" :key="item.id" :value="item.id">
          {{ item.name }}
        </option>
      </select>
      <strong v-else>Marktree</strong>
      <span class="mobile-subtitle">
        <template v-if="activeWorkspace?.git">
          {{ activeWorktree?.branch ?? $t('app.detachedHead') }} ·
          <WorkspaceSyncControls
            compact
            :pending="Boolean(pendingOperation)"
            :syncing="syncing"
            @abort="$emit('abort')"
            @sync="$emit('sync')"
          />
        </template>
        <template v-else>{{ $t('app.localWorkspace') }}</template>
      </span>
    </div>
    <button :aria-label="$t('app.commandPalette')" @click="$emit('commandPalette')">
      <Search :size="19" />
    </button>
    <button
      v-if="activeWorkspace"
      class="workspace-more-button"
      :aria-label="$t('app.workspaceSettings')"
      @click="$emit('settings')"
    >
      <MoreHorizontal :size="20" />
    </button>
  </header>

  <header v-else-if="activeWorkspace" class="workspace-topbar">
    <div class="branch-status">
      <strong>{{ activeWorkspace.name }}</strong>
      <template v-if="activeWorkspace.git">
        <span>{{ activeStatus?.branch ?? $t('app.detachedHead') }}</span>
        <i v-if="activeStatus?.ahead">↑ {{ activeStatus.ahead }}</i>
        <i v-if="activeStatus?.behind">↓ {{ activeStatus.behind }}</i>
      </template>
      <span v-else>{{ $t('app.localWorkspace') }}</span>
    </div>
    <div class="topbar-actions">
      <button @click="$emit('commandPalette')">
        <Search :size="16" /> {{ $t('app.commandPalette') }}
      </button>
      <button :disabled="loading" @click="$emit('refresh')">
        <RefreshCw :size="16" /> {{ $t('app.refresh') }}
      </button>
      <button
        v-if="activeWorkspace.git"
        class="advanced-git-button"
        :disabled="loading || Boolean(pendingOperation)"
        @click="$emit('toggleGitPanel')"
      >
        <GitBranch :size="16" />
        {{
          activeStatus?.changedCount
            ? $t('app.changed', { count: activeStatus.changedCount })
            : $t('app.advancedGit')
        }}
      </button>
      <WorkspaceSyncControls
        v-if="activeWorkspace.git"
        :pending="Boolean(pendingOperation)"
        :syncing="syncing"
        @abort="$emit('abort')"
        @sync="$emit('sync')"
      />
      <button
        class="icon-only workspace-more-button"
        :aria-label="$t('app.workspaceSettings')"
        @click="$emit('settings')"
      >
        <MoreHorizontal :size="18" />
      </button>
    </div>
  </header>
</template>
