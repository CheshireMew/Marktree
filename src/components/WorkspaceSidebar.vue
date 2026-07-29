<script setup lang="ts">
import {
  Box,
  ChevronDown,
  File,
  FileCode2,
  FileImage,
  FileText,
  FolderTree,
  GitBranch,
  Plus,
  Search,
  SquareArrowOutUpRight,
} from 'lucide-vue-next'

import type {
  DocumentDescriptor,
  RepositoryDescriptor,
  WorktreeDescriptor,
  WorktreeSearchResult,
} from '@/types'

defineProps<{
  repository: RepositoryDescriptor
  activeWorktree?: WorktreeDescriptor
  documents: DocumentDescriptor[]
  searchQuery: string
  searchResults?: WorktreeSearchResult[]
  mobile?: boolean
}>()

defineEmits<{
  'update:searchQuery': [value: string]
  search: []
  selectWorktree: [worktree: WorktreeDescriptor]
  openFile: [path: string]
  newFile: []
  newWorktree: []
  openWindow: [worktree: WorktreeDescriptor]
  addRepository: []
  openSearchResult: [result: WorktreeSearchResult]
}>()

function fileIcon(document: DocumentDescriptor) {
  if (document.kind === 'markdown') return FileText
  if (document.kind === 'image') return FileImage
  if (document.kind === 'text') return FileCode2
  return File
}
</script>

<template>
  <aside class="workspace-sidebar">
    <header class="sidebar-header">
      <div>
        <strong>{{ repository.name }}</strong>
        <span>{{ activeWorktree?.branch ?? $t('app.detachedHead') }}</span>
      </div>
      <button v-if="mobile" class="sidebar-add-repository" @click="$emit('addRepository')">
        <Plus :size="18" />
      </button>
      <GitBranch v-else :size="18" />
    </header>

    <section v-if="!mobile" class="sidebar-section worktree-section">
      <div class="section-title">
        <span><Box :size="15" /> {{ $t('app.worktrees') }}</span>
        <button :title="$t('app.worktreeNew')" @click="$emit('newWorktree')">
          <Plus :size="16" />
        </button>
      </div>
      <button
        v-for="worktree in repository.worktrees"
        :key="worktree.path"
        class="worktree-row"
        :class="{ active: worktree.path === activeWorktree?.path }"
        @click="$emit('selectWorktree', worktree)"
      >
        <span class="worktree-main">
          <FolderTree :size="16" />
          <span>
            <b>{{ worktree.isMain ? repository.name : worktree.name }}</b>
            <small>{{ worktree.branch ?? $t('app.detachedHead') }}</small>
          </span>
        </span>
        <span class="worktree-actions">
          <i v-if="worktree.status.changedCount">{{ worktree.status.changedCount }}</i>
          <button
            :title="$t('app.separateWindow')"
            @click.stop="$emit('openWindow', worktree)"
          >
            <SquareArrowOutUpRight :size="14" />
          </button>
        </span>
      </button>
    </section>

    <section class="sidebar-section file-section">
      <div class="section-title">
        <span><ChevronDown :size="15" /> {{ $t('app.files') }}</span>
        <button :title="$t('app.newDocument')" @click="$emit('newFile')">
          <Plus :size="16" />
        </button>
      </div>
      <label class="search-box">
        <Search :size="15" />
        <input
          :value="searchQuery"
          :placeholder="$t('app.search')"
          @input="$emit('update:searchQuery', ($event.target as HTMLInputElement).value)"
          @keyup.enter="$emit('search')"
        />
      </label>
      <div class="file-list">
        <button
          v-for="document in documents"
          :key="document.path"
          class="file-row"
          :class="{ muted: document.readOnly }"
          :title="document.path"
          @click="$emit('openFile', document.path)"
        >
          <component :is="fileIcon(document)" :size="16" />
          <span>
            <b>{{ document.name }}</b>
            <small v-if="document.path.includes('/')">{{
              document.path.slice(0, document.path.lastIndexOf('/'))
            }}</small>
          </span>
          <i
            v-if="document.gitStatus"
            class="file-state"
            :class="document.gitStatus.worktreeStatus"
          >
            {{
              document.gitStatus.untracked
                ? 'U'
                : document.gitStatus.worktreeStatus === 'modified'
                  ? 'M'
                  : '•'
            }}
          </i>
        </button>
        <div
          v-if="searchQuery && searchResults?.some((result) => result.root !== activeWorktree?.path)"
          class="cross-search-title"
        >
          {{ $t('app.otherWorktrees') }}
        </div>
        <button
          v-for="result in searchResults?.filter((item) => item.root !== activeWorktree?.path)"
          :key="`${result.root}-${result.path}`"
          class="file-row cross-search-row"
          @click="$emit('openSearchResult', result)"
        >
          <Search :size="15" />
          <span><b>{{ result.path }}</b><small>{{ result.worktree }}</small></span>
        </button>
      </div>
    </section>
  </aside>
</template>
