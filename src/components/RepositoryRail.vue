<script setup lang="ts">
import { FolderGit2, Moon, Plus, Sun } from 'lucide-vue-next'

import type { RepositoryDescriptor } from '@/types'

defineProps<{
  repositories: RepositoryDescriptor[]
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
  <aside class="repository-rail">
    <div class="brand-mark" title="Marktree">M</div>
    <nav class="repository-list" :aria-label="$t('app.repositories')">
      <button
        v-for="repository in repositories"
        :key="repository.id"
        class="repository-button"
        :class="{ active: repository.id === activeId }"
        :title="repository.name"
        @click="$emit('select', repository.id)"
      >
        <FolderGit2 :size="20" />
        <span>{{ repository.name.slice(0, 1).toUpperCase() }}</span>
        <i
          v-if="repository.status.changedCount || repository.status.stagedCount"
          class="change-dot"
        />
      </button>
      <button class="repository-button subtle" :title="$t('app.addRepository')" @click="$emit('add')">
        <Plus :size="21" />
      </button>
    </nav>
    <button class="rail-theme-button" :title="$t('app.theme')" @click="$emit('toggleTheme')">
      <Sun v-if="dark" :size="19" />
      <Moon v-else :size="19" />
    </button>
  </aside>
</template>
