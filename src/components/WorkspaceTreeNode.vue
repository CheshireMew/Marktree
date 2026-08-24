<script setup lang="ts">
import {
  ChevronDown,
  ChevronRight,
  File,
  FileAudio,
  FileCode2,
  FileImage,
  FileType2,
  FileText,
  FileVideo,
  Folder,
  FolderOpen,
} from 'lucide-vue-next'

import type { WorkspaceEntry } from '@/types'

export interface WorkspaceTreeItem {
  entry: WorkspaceEntry
  children: WorkspaceTreeItem[]
}

const props = defineProps<{
  item: WorkspaceTreeItem
  depth: number
  expanded: Set<string>
  recursive?: boolean
  activePath?: string
}>()

const emit = defineEmits<{
  toggle: [path: string]
  open: [path: string]
  context: [event: MouseEvent, entry: WorkspaceEntry]
  move: [sourcePath: string, destinationPath: string]
}>()

function iconFor(entry: WorkspaceEntry) {
  if (entry.entryType === 'directory') {
    return props.expanded.has(entry.path) ? FolderOpen : Folder
  }
  if (entry.fileKind === 'markdown') return FileText
  if (entry.fileKind === 'image') return FileImage
  if (entry.fileKind === 'pdf') return FileType2
  if (entry.fileKind === 'audio') return FileAudio
  if (entry.fileKind === 'video') return FileVideo
  if (entry.fileKind === 'text') return FileCode2
  return File
}

function activate() {
  if (props.item.entry.entryType === 'directory') emit('toggle', props.item.entry.path)
  else emit('open', props.item.entry.path)
}

function startDrag(event: DragEvent) {
  event.dataTransfer?.setData('application/x-marktree-path', props.item.entry.path)
  if (event.dataTransfer) event.dataTransfer.effectAllowed = 'move'
}

function dropInto(event: DragEvent) {
  if (props.item.entry.entryType !== 'directory') return
  const sourcePath = event.dataTransfer?.getData('application/x-marktree-path')
  if (!sourcePath || sourcePath === props.item.entry.path) return
  const name = sourcePath.split('/').at(-1) ?? sourcePath
  emit('move', sourcePath, `${props.item.entry.path}/${name}`)
}
</script>

<template>
  <div class="workspace-tree-node">
    <button
      class="file-row tree-row"
      :class="{
        muted: item.entry.readOnly && item.entry.fileKind !== 'image',
        active: item.entry.entryType === 'file' && item.entry.path === activePath,
      }"
      :style="{ '--tree-depth': depth }"
      :title="item.entry.path"
      :data-workspace-path="item.entry.path"
      draggable="true"
      @click="activate"
      @contextmenu.prevent="$emit('context', $event, item.entry)"
      @dragstart="startDrag"
      @dragover.prevent
      @drop.prevent.stop="dropInto"
    >
      <component
        :is="expanded.has(item.entry.path) ? ChevronDown : ChevronRight"
        v-if="item.entry.entryType === 'directory'"
        class="tree-chevron"
        :size="13"
      />
      <span v-else class="tree-chevron" />
      <component :is="iconFor(item.entry)" :size="16" />
      <span class="tree-label">
        <b>{{ item.entry.name }}</b>
      </span>
      <i
        v-if="item.entry.gitStatus"
        class="file-state"
        :class="item.entry.gitStatus.worktreeStatus"
      >
        {{
          item.entry.gitStatus.untracked
            ? 'U'
            : item.entry.gitStatus.worktreeStatus === 'modified'
              ? 'M'
              : item.entry.gitStatus.worktreeStatus === 'deleted'
                ? 'D'
                : '•'
        }}
      </i>
    </button>
    <template v-if="props.recursive !== false && item.entry.entryType === 'directory' && expanded.has(item.entry.path)">
      <WorkspaceTreeNode
        v-for="child in item.children"
        :key="child.entry.path"
        :item="child"
        :depth="depth + 1"
        :expanded="expanded"
        :active-path="activePath"
        @toggle="$emit('toggle', $event)"
        @open="$emit('open', $event)"
        @context="(event, entry) => $emit('context', event, entry)"
        @move="(source, destination) => $emit('move', source, destination)"
      />
    </template>
  </div>
</template>
