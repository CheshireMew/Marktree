<script setup lang="ts">
import { ChevronDown, FilePlus2, FolderPlus, Plus, Search, Star } from 'lucide-vue-next'
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'

import WorkspaceTreeNode, {
  type WorkspaceTreeItem,
} from '@/components/WorkspaceTreeNode.vue'
import {
  restoredWorkspaceExpansion,
  saveWorkspaceExpansion,
} from '@/lib/workspaceUiState'
import type { WorkspaceDescriptor, WorkspaceEntry, WorktreeSearchResult } from '@/types'

export interface WorkspaceEntryActionRequest {
  action: 'newFolder' | 'rename' | 'duplicate' | 'trash'
  directory: string
  sourcePath?: string
  entryName?: string
  entryType?: WorkspaceEntry['entryType']
  suggestedName?: string
}

const props = defineProps<{
  workspace: WorkspaceDescriptor
  entries: WorkspaceEntry[]
  favorites?: WorkspaceEntry[]
  searchQuery: string
  searchResults?: WorktreeSearchResult[]
  searching?: boolean
  activePath?: string
  mobile?: boolean
}>()

const emit = defineEmits<{
  'update:searchQuery': [value: string]
  search: []
  openSearchResult: [result: WorktreeSearchResult]
  openFile: [path: string]
  newFile: [directory: string]
  requestEntryAction: [request: WorkspaceEntryActionRequest]
  moveEntry: [sourcePath: string, destinationPath: string]
  openSystem: [path: string]
  addWorkspace: []
  toggleFavorite: [path: string]
}>()
const { t } = useI18n()

const expanded = ref(restoredWorkspaceExpansion(props.workspace.root))
const sidebar = ref<HTMLElement>()
const fileList = ref<HTMLElement>()
const context = ref<{ x: number; y: number; entry: WorkspaceEntry }>()
const contextMenu = ref<HTMLElement>()
const scrollTop = ref(0)
const viewportHeight = ref(0)
const TREE_ROW_HEIGHT = 37
const TREE_OVERSCAN = 8
let treeResizeObserver: ResizeObserver | undefined

const childrenByParent = computed(() => {
  const groups = new Map<string, WorkspaceEntry[]>()
  const directories = new Set(
    props.entries
      .filter((entry) => entry.entryType === 'directory')
      .map((entry) => entry.path),
  )
  for (const entry of props.entries) {
    const candidate = entry.path.split('/').slice(0, -1).join('/')
    const parent = directories.has(candidate) ? candidate : ''
    const children = groups.get(parent)
    if (children) children.push(entry)
    else groups.set(parent, [entry])
  }
  for (const children of groups.values()) {
    children.sort(
      (left, right) =>
        Number(left.entryType === 'file') - Number(right.entryType === 'file') ||
        left.name.localeCompare(right.name),
    )
  }
  return groups
})

const visibleTree = computed<Array<{ item: WorkspaceTreeItem; depth: number }>>(() => {
  const rows: Array<{ item: WorkspaceTreeItem; depth: number }> = []
  const append = (parent: string, depth: number) => {
    for (const entry of childrenByParent.value.get(parent) ?? []) {
      rows.push({ item: { entry, children: [] }, depth })
      if (entry.entryType === 'directory' && expanded.value.has(entry.path)) {
        append(entry.path, depth + 1)
      }
    }
  }
  append('', 0)
  return rows
})

const virtualTree = computed(() => {
  const start = Math.max(0, Math.floor(scrollTop.value / TREE_ROW_HEIGHT) - TREE_OVERSCAN)
  const visibleCount = Math.ceil(viewportHeight.value / TREE_ROW_HEIGHT) + TREE_OVERSCAN * 2
  const end = Math.min(visibleTree.value.length, start + visibleCount)
  return {
    start,
    rows: visibleTree.value.slice(start, end),
    top: start * TREE_ROW_HEIGHT,
    bottom: Math.max(0, (visibleTree.value.length - end) * TREE_ROW_HEIGHT),
  }
})

onMounted(() => {
  if (!fileList.value) return
  if (typeof ResizeObserver !== 'undefined') {
    treeResizeObserver = new ResizeObserver(([entry]) => {
      viewportHeight.value = entry?.contentRect.height ?? fileList.value?.clientHeight ?? 480
    })
    treeResizeObserver.observe(fileList.value)
  }
  viewportHeight.value = fileList.value.clientHeight
  document.addEventListener('pointerdown', dismissContextMenu, true)
  window.addEventListener('resize', closeContextMenu)
  window.addEventListener('scroll', closeContextMenu, true)
  window.addEventListener('keydown', closeContextMenuOnEscape)
})

onBeforeUnmount(() => {
  treeResizeObserver?.disconnect()
  document.removeEventListener('pointerdown', dismissContextMenu, true)
  window.removeEventListener('resize', closeContextMenu)
  window.removeEventListener('scroll', closeContextMenu, true)
  window.removeEventListener('keydown', closeContextMenuOnEscape)
})

watch(
  () => props.workspace.root,
  (root) => {
    expanded.value = restoredWorkspaceExpansion(root)
    scrollTop.value = 0
    if (fileList.value) fileList.value.scrollTop = 0
  },
)

function toggle(path: string) {
  const next = new Set(expanded.value)
  if (next.has(path)) next.delete(path)
  else next.add(path)
  expanded.value = next
  saveWorkspaceExpansion(props.workspace.root, next)
}

async function showContext(event: MouseEvent, entry: WorkspaceEntry) {
  context.value = { x: event.clientX, y: event.clientY, entry }
  await nextTick()
  const rect = contextMenu.value?.getBoundingClientRect()
  if (!rect || !context.value) return
  const margin = 8
  context.value = {
    ...context.value,
    x: Math.max(margin, Math.min(event.clientX, window.innerWidth - rect.width - margin)),
    y: Math.max(margin, Math.min(event.clientY, window.innerHeight - rect.height - margin)),
  }
}

function dismissContextMenu(event: PointerEvent) {
  if (contextMenu.value?.contains(event.target as Node)) return
  context.value = undefined
}

function closeContextMenu() {
  context.value = undefined
}

function closeContextMenuOnEscape(event: KeyboardEvent) {
  if (event.key === 'Escape') closeContextMenu()
}

function directoryFor(entry: WorkspaceEntry) {
  return entry.entryType === 'directory'
    ? entry.path
    : entry.path.split('/').slice(0, -1).join('/')
}

function newFile(entry?: WorkspaceEntry) {
  context.value = undefined
  emit('newFile', entry ? directoryFor(entry) : '')
}

function newFolder(entry?: WorkspaceEntry) {
  const directory = entry ? directoryFor(entry) : ''
  context.value = undefined
  emit('requestEntryAction', { action: 'newFolder', directory })
}

function renameEntry(entry: WorkspaceEntry) {
  context.value = undefined
  const parent = entry.path.split('/').slice(0, -1).join('/')
  emit('requestEntryAction', {
    action: 'rename',
    directory: parent,
    sourcePath: entry.path,
    entryName: entry.name,
    entryType: entry.entryType,
    suggestedName: entry.name,
  })
}

function duplicateEntry(entry: WorkspaceEntry) {
  const parent = entry.path.split('/').slice(0, -1).join('/')
  const extensionIndex = entry.entryType === 'file' ? entry.name.lastIndexOf('.') : -1
  const stem = extensionIndex > 0 ? entry.name.slice(0, extensionIndex) : entry.name
  const extension = extensionIndex > 0 ? entry.name.slice(extensionIndex) : ''
  const suggested = `${stem} ${t('app.copySuffix')}${extension}`
  context.value = undefined
  emit('requestEntryAction', {
    action: 'duplicate',
    directory: parent,
    sourcePath: entry.path,
    entryName: entry.name,
    entryType: entry.entryType,
    suggestedName: suggested,
  })
}

function deleteEntry(entry: WorkspaceEntry) {
  context.value = undefined
  emit('requestEntryAction', {
    action: 'trash',
    directory: directoryFor(entry),
    sourcePath: entry.path,
    entryName: entry.name,
    entryType: entry.entryType,
  })
}

function dropAtRoot(event: DragEvent) {
  const sourcePath = event.dataTransfer?.getData('application/x-marktree-path')
  if (!sourcePath || !sourcePath.includes('/')) return
  emit('moveEntry', sourcePath, sourcePath.split('/').at(-1) ?? sourcePath)
}

async function revealPath(path: string) {
  const parts = path.split('/')
  const next = new Set(expanded.value)
  for (let index = 1; index <= parts.length; index += 1) {
    const candidate = parts.slice(0, index).join('/')
    if (props.entries.some((entry) => entry.path === candidate && entry.entryType === 'directory')) {
      next.add(candidate)
    }
  }
  expanded.value = next
  saveWorkspaceExpansion(props.workspace.root, next)
  await nextTick()
  const index = visibleTree.value.findIndex((row) => row.item.entry.path === path)
  if (index >= 0 && fileList.value) {
    fileList.value.scrollTop = index * TREE_ROW_HEIGHT
    scrollTop.value = fileList.value.scrollTop
    await nextTick()
  }
  const target = [...(sidebar.value?.querySelectorAll<HTMLElement>('[data-workspace-path]') ?? [])]
    .find((element) => element.dataset.workspacePath === path)
  target?.scrollIntoView({ block: 'nearest' })
  target?.focus()
}

defineExpose({ revealPath })
</script>

<template>
  <aside
    ref="sidebar"
    class="workspace-sidebar"
    @click="context = undefined"
    @dragover.prevent
    @drop.prevent="dropAtRoot"
  >
    <header class="sidebar-header">
      <div>
        <strong>{{ workspace.name }}</strong>
        <span>{{ workspace.git ? $t('app.gitWorkspace') : $t('app.localWorkspace') }}</span>
      </div>
      <button v-if="mobile" class="sidebar-add-workspace" :aria-label="$t('app.addWorkspace')" @click="$emit('addWorkspace')">
        <Plus :size="18" />
      </button>
      <ChevronDown v-else :size="18" />
    </header>

    <section class="sidebar-section file-section">
      <div v-if="favorites?.length" class="favorite-files">
        <div class="section-title">
          <span><Star :size="14" /> {{ $t('app.favorites') }}</span>
        </div>
        <button
          v-for="entry in favorites"
          :key="entry.path"
          class="file-row favorite-file-row"
          :title="entry.path"
          @click="$emit('openFile', entry.path)"
        >
          <Star :size="13" fill="currentColor" />
          <span>{{ entry.name }}</span>
        </button>
      </div>
      <div class="section-title">
        <span><ChevronDown :size="15" /> {{ $t('app.files') }}</span>
        <span class="tree-create-actions">
          <button :title="$t('app.newDocument')" :aria-label="$t('app.newDocument')" @click.stop="newFile()">
            <FilePlus2 :size="16" />
          </button>
          <button :title="$t('app.newFolder')" :aria-label="$t('app.newFolder')" @click.stop="newFolder()">
            <FolderPlus :size="16" />
          </button>
        </span>
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
      <div
        ref="fileList"
        class="file-list"
        @scroll="scrollTop = ($event.currentTarget as HTMLElement).scrollTop"
      >
        <template v-if="searchQuery.trim()">
          <p v-if="searching" class="sidebar-search-state" role="status">
            {{ $t('app.searching') }}
          </p>
          <template v-else-if="searchResults?.length">
            <p class="sidebar-search-count">
              {{ $t('app.searchResultCount', { count: searchResults.length }) }}
            </p>
            <button
              v-for="result in searchResults"
              :key="`${result.root}:${result.path}:${result.line ?? 0}:${result.matchType}`"
              class="sidebar-search-row"
              :class="{ active: result.path === activePath }"
              :title="result.path"
              @click="$emit('openSearchResult', result)"
            >
              <strong>{{ result.path.split('/').at(-1) }}</strong>
              <span>{{ result.line ? `${result.path}:${result.line}` : result.path }}</span>
              <small v-if="result.matchType === 'content'">{{ result.snippet }}</small>
            </button>
          </template>
          <p v-else class="sidebar-search-state">{{ $t('app.noSearchResults') }}</p>
        </template>
        <template v-else>
          <div v-if="virtualTree.top" :style="{ height: `${virtualTree.top}px` }" />
          <WorkspaceTreeNode
            v-for="row in virtualTree.rows"
            :key="row.item.entry.path"
            :item="row.item"
            :depth="row.depth"
            :expanded="expanded"
            :active-path="activePath"
            :recursive="false"
            @toggle="toggle"
            @open="$emit('openFile', $event)"
            @context="showContext"
            @move="(source, destination) => $emit('moveEntry', source, destination)"
          />
          <div v-if="virtualTree.bottom" :style="{ height: `${virtualTree.bottom}px` }" />
        </template>
      </div>
    </section>
    <div
      v-if="context"
      ref="contextMenu"
      class="entry-context-menu"
      :style="{ left: `${context.x}px`, top: `${context.y}px` }"
      @click.stop
    >
      <button @click="newFile(context.entry)">{{ $t('app.newDocument') }}</button>
      <button @click="newFolder(context.entry)">{{ $t('app.newFolder') }}</button>
      <button @click="renameEntry(context.entry)">{{ $t('app.rename') }}</button>
      <button @click="duplicateEntry(context.entry)">{{ $t('app.createDuplicate') }}</button>
      <button
        v-if="context.entry.entryType === 'file'"
        @click="$emit('toggleFavorite', context.entry.path); context = undefined"
      >
        {{
          favorites?.some((entry) => entry.path === context?.entry.path)
            ? $t('app.removeFavorite')
            : $t('app.addFavorite')
        }}
      </button>
      <button
        v-if="context.entry.entryType === 'file'"
        @click="$emit('openSystem', context.entry.path); context = undefined"
      >
        {{ $t('app.openWithSystem') }}
      </button>
      <button class="danger" @click="deleteEntry(context.entry)">
        {{ $t('app.moveToTrash') }}
      </button>
    </div>
  </aside>
</template>
