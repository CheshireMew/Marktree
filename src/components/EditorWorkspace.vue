<script setup lang="ts">
import {
  BookOpen,
  ChevronLeft,
  ChevronRight,
  FileText,
  GitCompareArrows,
  ListTree,
  LockKeyhole,
  Printer,
  Star,
  X,
} from 'lucide-vue-next'
import { computed, nextTick, onBeforeUnmount, ref, watch } from 'vue'

import type { EditorTab, WorkspaceEntry, WorkspaceImageLoader } from '@/types'
import { analyzeMarkdownOutline } from '@/lib/editor/documentAnalysis'
import { markdownOutline, type MarkdownOutlineEntry } from '@/lib/editor/outline'
import MarkdownEditor from './MarkdownEditor.vue'

const editor = ref<InstanceType<typeof MarkdownEditor>>()

const props = defineProps<{
  tabs: EditorTab[]
  activeTab?: EditorTab
  activeKey?: string
  entries: WorkspaceEntry[]
  favorite?: boolean
  dark: boolean
  canCompare?: boolean
  loadWorkspaceImage: WorkspaceImageLoader
  documentCharacterLimit: number
}>()

const emit = defineEmits<{
  activate: [key: string]
  close: [tab: EditorTab]
  updateContent: [content: string]
  asset: [file: File, cursor: number]
  diff: []
  openPath: [path: string]
  toggleFavorite: []
  revealPath: [path: string]
  error: [message: string]
}>()

const readingMode = ref(false)
const outlineOpen = ref(false)
const headings = ref<MarkdownOutlineEntry[]>([])
const documentStats = ref<{ words: number | null; lines: number }>({ words: null, lines: 0 })
let outlineGeneration = 0
let outlineTimer: number | undefined

watch(
  () => props.activeKey,
  () => {
    readingMode.value = false
    outlineOpen.value = false
    headings.value = []
    documentStats.value = { words: null, lines: 0 }
    outlineGeneration += 1
  },
)

function key(tab: EditorTab) {
  return `${tab.root}\n${tab.path}`
}

const activeIsMarkdown = computed(() =>
  /\.(?:md|markdown|mdx)$/i.test(props.activeTab?.path ?? ''),
)

const breadcrumbs = computed(() => {
  const parts = props.activeTab?.path.split('/') ?? []
  return parts.map((label, index) => ({
    label,
    path: parts.slice(0, index + 1).join('/'),
    file: index === parts.length - 1,
  }))
})

const navigableDocuments = computed(() =>
  props.entries
    .filter(
      (entry) =>
        entry.entryType === 'file' &&
        (entry.fileKind === 'markdown' || entry.fileKind === 'text'),
    )
    .sort((left, right) => left.path.localeCompare(right.path)),
)

const navigation = computed(() => {
  const index = navigableDocuments.value.findIndex(
    (entry) => entry.path === props.activeTab?.path,
  )
  return {
    previous: index > 0 ? navigableDocuments.value[index - 1] : undefined,
    next:
      index >= 0 && index < navigableDocuments.value.length - 1
        ? navigableDocuments.value[index + 1]
        : undefined,
  }
})

const largeDocument = computed(
  () => (props.activeTab?.content.length ?? 0) >= 2 * 1024 * 1024,
)

watch(largeDocument, (large) => {
  if (large) readingMode.value = false
})

async function refreshOutline() {
  if (!activeIsMarkdown.value || !props.activeTab) return
  const generation = ++outlineGeneration
  const source = props.activeTab.content
  if (source.length < 256 * 1024) {
    headings.value = markdownOutline(source)
    return
  }
  const result = await analyzeMarkdownOutline(source)
  if (generation === outlineGeneration && source === props.activeTab?.content) {
    headings.value = result
  }
}

function toggleOutline() {
  outlineOpen.value = !outlineOpen.value
  if (outlineOpen.value) void refreshOutline()
}

watch(
  () => props.activeTab?.content,
  () => {
    if (!outlineOpen.value) return
    if (outlineTimer) window.clearTimeout(outlineTimer)
    outlineTimer = window.setTimeout(() => void refreshOutline(), 500)
  },
)

onBeforeUnmount(() => {
  if (outlineTimer) window.clearTimeout(outlineTimer)
})

async function printDocument() {
  if (largeDocument.value) {
    window.print()
    return
  }
  readingMode.value = true
  await nextTick()
  window.print()
}

function focusAtLine(line: number, column = 1) {
  editor.value?.focusAtLine(line, column)
}

function openSnippetManager() {
  editor.value?.openSnippetManager()
}

function insertText(text: string) {
  editor.value?.insertText(text)
}

defineExpose({ focusAtLine, insertText, openSnippetManager })
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
          <nav class="document-breadcrumbs" :aria-label="$t('app.breadcrumbs')">
            <template v-for="(part, index) in breadcrumbs" :key="part.path">
              <span v-if="index">/</span>
              <button
                :disabled="part.file"
                @click="!part.file && emit('revealPath', part.path)"
              >
                {{ part.label }}
              </button>
            </template>
          </nav>
          <em v-if="activeTab.readOnly"><LockKeyhole :size="13" /> {{ $t('app.readOnly') }}</em>
        </div>
        <div class="editor-actions">
          <button
            v-if="activeIsMarkdown"
            class="icon-text-button"
            :class="{ active: outlineOpen }"
            @click="toggleOutline"
          >
            <ListTree :size="15" /> {{ $t('app.outline') }}
          </button>
          <button
            class="icon-text-button"
            :disabled="!navigation.previous"
            :title="navigation.previous ? $t('app.previousDocument', { path: navigation.previous.path }) : $t('app.previousDocumentUnavailable')"
            :aria-label="$t('app.previousDocumentAction')"
            @click="navigation.previous && emit('openPath', navigation.previous.path)"
          >
            <ChevronLeft :size="15" />
          </button>
          <button
            class="icon-text-button"
            :disabled="!navigation.next"
            :title="navigation.next ? $t('app.nextDocument', { path: navigation.next.path }) : $t('app.nextDocumentUnavailable')"
            :aria-label="$t('app.nextDocumentAction')"
            @click="navigation.next && emit('openPath', navigation.next.path)"
          >
            <ChevronRight :size="15" />
          </button>
          <button
            class="icon-text-button"
            :class="{ active: favorite }"
            :title="favorite ? $t('app.removeFavorite') : $t('app.addFavorite')"
            :aria-label="favorite ? $t('app.removeFavorite') : $t('app.addFavorite')"
            @click="emit('toggleFavorite')"
          >
            <Star :size="15" :fill="favorite ? 'currentColor' : 'none'" />
          </button>
          <button
            v-if="activeIsMarkdown && !largeDocument"
            class="icon-text-button"
            :class="{ active: readingMode }"
            @click="readingMode = !readingMode"
          >
            <BookOpen :size="15" /> {{ $t('app.readingView') }}
          </button>
          <button v-if="activeIsMarkdown" class="icon-text-button" @click="printDocument">
            <Printer :size="15" /> {{ $t('app.print') }}
          </button>
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
      <div class="editor-body">
        <nav v-if="outlineOpen" class="document-outline">
          <strong>{{ $t('app.outline') }}</strong>
          <button
            v-for="heading in headings"
            :key="heading.position"
            :style="{ paddingLeft: `${10 + (heading.level - 1) * 14}px` }"
            @click="editor?.focusAt(heading.position)"
          >
            {{ heading.text }}
          </button>
        </nav>
        <MarkdownEditor
          ref="editor"
          :key="key(activeTab)"
          :model-value="activeTab.content"
          :read-only="activeTab.readOnly"
          :reading-mode="readingMode"
          :dark="dark"
          :root="activeTab.root"
          :path="activeTab.path"
          :markdown="activeIsMarkdown"
          :large-document="largeDocument"
          :document-character-limit="documentCharacterLimit"
          :link-candidates="entries"
          :load-workspace-image="loadWorkspaceImage"
          @update:model-value="$emit('updateContent', $event)"
          @metrics="documentStats = $event"
          @limit="$emit('error', $t('app.openDocumentLimit'))"
          @asset="(file, cursor) => $emit('asset', file, cursor)"
        />
      </div>
      <footer class="document-stats">
        {{ documentStats.words === null ? '—' : $t('app.wordCount', { count: documentStats.words }) }} ·
        {{ $t('app.lineCount', { count: documentStats.lines }) }}
      </footer>
    </template>

    <div v-else class="empty-editor">
      <img :src="'/marktree.svg'" alt="" />
      <h2>{{ $t('app.noDocument') }}</h2>
      <p>{{ $t('app.noDocumentHint') }}</p>
    </div>
  </main>
</template>
