<script setup lang="ts">
import { Folder, Moon, Plus, Sun } from 'lucide-vue-next'

import type { WorkspaceDescriptor } from '@/types'

defineProps<{
  workspaces: WorkspaceDescriptor[]
  activeId?: string
  dark: boolean
}>()

defineEmits<{
  select: [id: string]
  add: []
  toggleTheme: []
}>()
</script>

<template>
  <aside class="workspace-rail">
    <div class="brand-mark" title="Marktree">M</div>
    <nav class="workspace-list" :aria-label="$t('app.workspaces')">
      <button
        v-for="workspace in workspaces"
        :key="workspace.id"
        class="workspace-button"
        :class="{ active: workspace.id === activeId }"
        :title="workspace.name"
        @click="$emit('select', workspace.id)"
      >
        <Folder :size="20" />
        <span>{{ workspace.name.slice(0, 1).toUpperCase() }}</span>
        <i
          v-if="workspace.git && (workspace.git.status.changedCount || workspace.git.status.stagedCount)"
          class="change-dot"
        />
      </button>
      <button class="workspace-button subtle" :title="$t('app.addWorkspace')" @click="$emit('add')">
        <Plus :size="21" />
      </button>
    </nav>
    <button class="rail-theme-button" :title="$t('app.theme')" @click="$emit('toggleTheme')">
      <Sun v-if="dark" :size="19" />
      <Moon v-else :size="19" />
    </button>
  </aside>
</template>
