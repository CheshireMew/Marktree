<script setup lang="ts">
import { ChevronDown, FilePlus2, FolderPlus, Plus, Search } from 'lucide-vue-next'
import { computed, ref } from 'vue'

import WorkspaceTreeNode, {
  type WorkspaceTreeItem,
} from '@/components/WorkspaceTreeNode.vue'
import type { WorkspaceDescriptor, WorkspaceEntry } from '@/types'

const props = defineProps<{
  workspace: WorkspaceDescriptor
  entries: WorkspaceEntry[]
  searchQuery: string
  mobile?: boolean
}>()

const emit = defineEmits<{
  'update:searchQuery': [value: string]
  search: []
  openFile: [path: string]
  newFile: [directory: string]
  newFolder: [path: string]
  moveEntry: [sourcePath: string, destinationPath: string]
  trashEntry: [path: string]
  openSystem: [path: string]
  addWorkspace: []
}>()

const expanded = ref(new Set<string>())
const context = ref<{ x: number; y: number; entry: WorkspaceEntry }>()

const tree = computed(() => {
  const items = new Map<string, WorkspaceTreeItem>()
  for (const entry of props.entries) {
    items.set(entry.path, { entry, children: [] })
  }
  const roots: WorkspaceTreeItem[] = []
  for (const item of items.values()) {
    const parentPath = item.entry.path.split('/').slice(0, -1).join('/')
    const parent = items.get(parentPath)
    if (parent) parent.children.push(item)
    else roots.push(item)
  }
  const sort = (nodes: WorkspaceTreeItem[]) => {
    nodes.sort(
      (left, right) =>
        Number(left.entry.entryType === 'file') -
          Number(right.entry.entryType === 'file') ||
        left.entry.name.localeCompare(right.entry.name),
    )
    for (const node of nodes) sort(node.children)
  }
  sort(roots)
  return roots
})

function toggle(path: string) {
  const next = new Set(expanded.value)
  if (next.has(path)) next.delete(path)
  else next.add(path)
  expanded.value = next
}

function showContext(event: MouseEvent, entry: WorkspaceEntry) {
  context.value = { x: event.clientX, y: event.clientY, entry }
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
  const name = window.prompt('新文件夹名称')
  context.value = undefined
  if (!name?.trim()) return
  emit('newFolder', directory ? `${directory}/${name.trim()}` : name.trim())
}

function renameEntry(entry: WorkspaceEntry) {
  const name = window.prompt('新名称', entry.name)
  context.value = undefined
  if (!name?.trim() || name.trim() === entry.name) return
  const parent = entry.path.split('/').slice(0, -1).join('/')
  emit('moveEntry', entry.path, parent ? `${parent}/${name.trim()}` : name.trim())
}

function deleteEntry(entry: WorkspaceEntry) {
  context.value = undefined
  if (window.confirm(`确定把“${entry.name}”移入回收站吗？`)) {
    emit('trashEntry', entry.path)
  }
}

function dropAtRoot(event: DragEvent) {
  const sourcePath = event.dataTransfer?.getData('application/x-marktree-path')
  if (!sourcePath || !sourcePath.includes('/')) return
  emit('moveEntry', sourcePath, sourcePath.split('/').at(-1) ?? sourcePath)
}
</script>

<template>
  <aside
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
      <button v-if="mobile" class="sidebar-add-workspace" @click="$emit('addWorkspace')">
        <Plus :size="18" />
      </button>
      <ChevronDown v-else :size="18" />
    </header>

    <section class="sidebar-section file-section">
      <div class="section-title">
        <span><ChevronDown :size="15" /> {{ $t('app.files') }}</span>
        <span class="tree-create-actions">
          <button :title="$t('app.newDocument')" @click.stop="newFile()">
            <FilePlus2 :size="16" />
          </button>
          <button :title="$t('app.newFolder')" @click.stop="newFolder()">
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
      <div class="file-list">
        <WorkspaceTreeNode
          v-for="item in tree"
          :key="item.entry.path"
          :item="item"
          :depth="0"
          :expanded="expanded"
          @toggle="toggle"
          @open="$emit('openFile', $event)"
          @context="showContext"
          @move="(source, destination) => $emit('moveEntry', source, destination)"
        />
      </div>
    </section>
    <div
      v-if="context"
      class="entry-context-menu"
      :style="{ left: `${context.x}px`, top: `${context.y}px` }"
      @click.stop
    >
      <button @click="newFile(context.entry)">{{ $t('app.newDocument') }}</button>
      <button @click="newFolder(context.entry)">{{ $t('app.newFolder') }}</button>
      <button @click="renameEntry(context.entry)">{{ $t('app.rename') }}</button>
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
