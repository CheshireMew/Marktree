<script setup lang="ts">
import { FileText, GitCompareArrows, LockKeyhole, X } from 'lucide-vue-next'
import { computed, ref } from 'vue'

import type { EditorTab, WorkspaceImageLoader } from '@/types'
import MarkdownEditor from './MarkdownEditor.vue'

const editor = ref<InstanceType<typeof MarkdownEditor>>()

const props = defineProps<{
  tabs: EditorTab[]
  activeTab?: EditorTab
  activeKey?: string
  dark: boolean
  canCompare?: boolean
  loadWorkspaceImage: WorkspaceImageLoader
}>()

defineEmits<{
  activate: [key: string]
  close: [tab: EditorTab]
  updateContent: [content: string]
  asset: [file: File, cursor: number]
  diff: []
}>()

function key(tab: EditorTab) {
  return `${tab.root}\n${tab.path}`
}

const headings = computed(() => {
  if (!activeIsMarkdown.value) return []
  const content = props.activeTab?.content ?? ''
  const results: Array<{ text: string; level: number; position: number }> = []
  const pattern = /^(#{1,6})\s+(.+)$/gm
  for (const match of content.matchAll(pattern)) {
    results.push({
      level: match[1]?.length ?? 1,
      text: match[2]?.replace(/\s+#+\s*$/, '') ?? '',
      position: match.index,
    })
  }
  return results
})

const activeIsMarkdown = computed(() =>
  /\.(?:md|markdown|mdx)$/i.test(props.activeTab?.path ?? ''),
)
</script>

<template>
  <main class="editor-workspace">
    <div v-if="tabs.length" class="tab-strip" role="tablist">
      <div
        v-for="tab in tabs"
        :key="key(tab)"
        class="document-tab"
        :class="{ active: key(tab) === activeKey }"
        role="tab"
        tabindex="0"
        :aria-selected="key(tab) === activeKey"
        @click="$emit('activate', key(tab))"
        @keydown.enter="$emit('activate', key(tab))"
        @keydown.space.prevent="$emit('activate', key(tab))"
      >
        <FileText :size="14" />
        <span>{{ tab.title }}</span>
        <i v-if="tab.dirty" />
        <button
          type="button"
          :aria-label="$t('app.closeTab', { name: tab.title })"
          @click.stop="$emit('close', tab)"
        >
          <X :size="13" />
        </button>
      </div>
    </div>

    <template v-if="activeTab">
      <header class="editor-header">
        <div class="document-location">
          <span>{{ activeTab.path }}</span>
          <em v-if="activeTab.readOnly"><LockKeyhole :size="13" /> {{ $t('app.readOnly') }}</em>
        </div>
        <div class="editor-actions">
          <select
            v-if="headings.length"
            class="outline-select"
            value=""
            @change="
              editor?.focusAt(Number(($event.target as HTMLSelectElement).value));
              ($event.target as HTMLSelectElement).value = ''
            "
          >
            <option value="" disabled>{{ $t('app.headingNavigation') }}</option>
            <option
              v-for="heading in headings"
              :key="heading.position"
              :value="heading.position"
            >
              {{ '　'.repeat(heading.level - 1) }}{{ heading.text }}
            </option>
          </select>
          <span class="save-state" :class="{ error: activeTab.saveError }">
            {{
              activeTab.saveError
                ? activeTab.saveError
                : activeTab.saving
                  ? $t('app.saving')
                  : activeTab.dirty
                    ? $t('app.unsaved')
                    : $t('app.save')
            }}
          </span>
          <button
            v-if="activeTab.dirty || canCompare"
            class="icon-text-button"
            @click="$emit('diff')"
          >
            <GitCompareArrows :size="15" /> {{ $t('app.diff') }}
          </button>
        </div>
      </header>
      <MarkdownEditor
        ref="editor"
        :key="key(activeTab)"
        :model-value="activeTab.content"
        :read-only="activeTab.readOnly"
        :dark="dark"
        :root="activeTab.root"
        :path="activeTab.path"
        :markdown="activeIsMarkdown"
        :load-workspace-image="loadWorkspaceImage"
        @update:model-value="$emit('updateContent', $event)"
        @asset="(file, cursor) => $emit('asset', file, cursor)"
      />
    </template>

    <div v-else class="empty-editor">
      <img src="/marktree.svg" alt="" />
      <h2>{{ $t('app.noDocument') }}</h2>
      <p>{{ $t('app.noDocumentHint') }}</p>
    </div>
  </main>
</template>
