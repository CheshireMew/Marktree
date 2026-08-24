<script setup lang="ts">
import { Command, FileText, FolderOpen, Search } from 'lucide-vue-next'
import { computed, nextTick, ref, watch } from 'vue'

import { useDialogAccessibility } from '@/composables/useDialogAccessibility'
import type {
  CommandPaletteItem,
  UnsavedComparison,
  WorkspaceFilePreviewState,
} from '@/types'

const props = defineProps<{
  externalComparison?: UnsavedComparison
  commandPaletteOpen: boolean
  commandPaletteQuery: string
  commandPaletteResults: CommandPaletteItem[]
  commandPaletteSearching?: boolean
  commandPalettePathPrefix: string
  commandPaletteFileKind: 'all' | 'markdown' | 'text'
  commandPaletteModifiedDays: number
  filePreview?: WorkspaceFilePreviewState
}>()

const emit = defineEmits<{
  chooseExternalVersion: [choice: 'disk' | 'editor']
  closeCommandPalette: []
  updateCommandPaletteQuery: [value: string]
  updateCommandPalettePathPrefix: [value: string]
  updateCommandPaletteFileKind: [value: 'all' | 'markdown' | 'text']
  updateCommandPaletteModifiedDays: [value: number]
  chooseCommandPalette: [item: CommandPaletteItem]
  closeFilePreview: []
}>()

const commandInput = ref<HTMLInputElement>()
const commandResults = ref<HTMLElement>()
const externalSurface = ref<HTMLElement>()
const commandSurface = ref<HTMLElement>()
const previewSurface = ref<HTMLElement>()
const activeIndex = ref(0)
useDialogAccessibility(
  computed(() => props.externalComparison),
  externalSurface,
  () => undefined,
  { closeOnEscape: false },
)
useDialogAccessibility(
  computed(() => props.commandPaletteOpen),
  commandSurface,
  () => emit('closeCommandPalette'),
)
useDialogAccessibility(
  computed(() => props.filePreview),
  previewSurface,
  () => emit('closeFilePreview'),
)

watch(
  () => props.commandPaletteOpen,
  async (open) => {
    if (!open) return
    activeIndex.value = 0
    await nextTick()
    commandInput.value?.focus()
  },
)

watch(
  () => props.commandPaletteResults,
  () => {
    activeIndex.value = Math.min(
      activeIndex.value,
      Math.max(0, props.commandPaletteResults.length - 1),
    )
  },
)

watch(activeIndex, async (index) => {
  await nextTick()
  commandResults.value
    ?.querySelector<HTMLElement>(`[data-command-index="${index}"]`)
    ?.scrollIntoView({ block: 'nearest' })
})

function moveSelection(delta: number) {
  const length = props.commandPaletteResults.length
  if (!length) return
  activeIndex.value = (activeIndex.value + delta + length) % length
}

function chooseActive() {
  const item = props.commandPaletteResults[activeIndex.value]
  if (item) emit('chooseCommandPalette', item)
}

function highlightedDetail(item: CommandPaletteItem) {
  const query = props.commandPaletteQuery.trim().replace(/^>/, '').trim()
  if (!query) return [{ text: item.detail, match: false }]
  const index = item.detail.toLocaleLowerCase().indexOf(query.toLocaleLowerCase())
  if (index < 0) return [{ text: item.detail, match: false }]
  return [
    { text: item.detail.slice(0, index), match: false },
    { text: item.detail.slice(index, index + query.length), match: true },
    { text: item.detail.slice(index + query.length), match: false },
  ].filter((part) => part.text)
}
</script>

<template>
  <div v-if="externalComparison" class="modal-backdrop">
    <section
      ref="externalSurface"
      class="dialog comparison-dialog"
      role="dialog"
      aria-modal="true"
      aria-labelledby="external-comparison-title"
      tabindex="-1"
    >
      <header>
        <div>
          <h2 id="external-comparison-title">{{ $t('app.externalTitle') }}</h2>
          <p>{{ $t('app.externalBody') }}</p>
        </div>
      </header>
      <div class="comparison-grid">
        <article>
          <h3>{{ $t('app.disk') }}</h3>
          <pre>{{ externalComparison.disk }}</pre>
        </article>
        <article>
          <h3>{{ $t('app.editor') }}</h3>
          <pre>{{ externalComparison.editor }}</pre>
        </article>
      </div>
      <footer>
        <button @click="$emit('chooseExternalVersion', 'disk')">
          {{ $t('app.useDisk') }}
        </button>
        <button class="primary" @click="$emit('chooseExternalVersion', 'editor')">
          {{ $t('app.keepEditor') }}
        </button>
      </footer>
    </section>
  </div>

  <div
    v-if="commandPaletteOpen"
    class="modal-backdrop"
    @mousedown.self="$emit('closeCommandPalette')"
  >
    <section
      ref="commandSurface"
      class="command-palette-dialog"
      role="dialog"
      aria-modal="true"
      aria-labelledby="command-palette-title"
      tabindex="-1"
    >
      <h2 id="command-palette-title" class="visually-hidden">
        {{ $t('app.commandPalette') }}
      </h2>
      <label>
        <Search :size="18" />
        <input
          ref="commandInput"
          data-dialog-initial-focus
          role="combobox"
          aria-autocomplete="list"
          aria-expanded="true"
          aria-controls="command-palette-results"
          :aria-activedescendant="commandPaletteResults.length ? `command-palette-option-${activeIndex}` : undefined"
          :value="commandPaletteQuery"
          :placeholder="$t('app.commandPalettePlaceholder')"
          @input="
            $emit(
              'updateCommandPaletteQuery',
              ($event.target as HTMLInputElement).value,
            )
          "
          @keydown.down.prevent="moveSelection(1)"
          @keydown.up.prevent="moveSelection(-1)"
          @keydown.enter.prevent="chooseActive"
          @keydown.esc.prevent="$emit('closeCommandPalette')"
        />
        <kbd>Ctrl P</kbd>
      </label>
      <div
        v-if="commandPaletteQuery.trim() && !commandPaletteQuery.trim().startsWith('>')"
        class="command-search-filters"
      >
        <input
          :value="commandPalettePathPrefix"
          :placeholder="$t('app.searchPathPrefix')"
          @input="$emit('updateCommandPalettePathPrefix', ($event.target as HTMLInputElement).value)"
        />
        <select
          :value="commandPaletteFileKind"
          @change="$emit('updateCommandPaletteFileKind', ($event.target as HTMLSelectElement).value as 'all' | 'markdown' | 'text')"
        >
          <option value="all">{{ $t('app.allTextFiles') }}</option>
          <option value="markdown">Markdown</option>
          <option value="text">{{ $t('app.plainTextFiles') }}</option>
        </select>
        <select
          :value="commandPaletteModifiedDays"
          @change="$emit('updateCommandPaletteModifiedDays', Number(($event.target as HTMLSelectElement).value))"
        >
          <option :value="0">{{ $t('app.anyModifiedTime') }}</option>
          <option :value="7">{{ $t('app.modifiedWithinDays', { count: 7 }) }}</option>
          <option :value="30">{{ $t('app.modifiedWithinDays', { count: 30 }) }}</option>
        </select>
      </div>
      <div id="command-palette-results" ref="commandResults" class="command-palette-results" role="listbox">
        <button
          v-for="(item, index) in commandPaletteResults"
          :key="item.key"
          :id="`command-palette-option-${index}`"
          role="option"
          :aria-selected="index === activeIndex"
          :data-command-index="index"
          :class="{ active: index === activeIndex }"
          @mouseenter="activeIndex = index"
          @click="$emit('chooseCommandPalette', item)"
        >
          <Command v-if="item.kind === 'command'" :size="16" />
          <FolderOpen v-else-if="item.kind === 'workspace'" :size="16" />
          <FileText v-else :size="16" />
          <span>
            <strong>{{ item.title }}</strong>
            <small>
              <template
                v-for="(part, partIndex) in highlightedDetail(item)"
                :key="partIndex"
              >
                <mark v-if="part.match">{{ part.text }}</mark>
                <template v-else>{{ part.text }}</template>
              </template>
            </small>
          </span>
          <em>
            {{
              item.kind === 'command'
                ? $t('app.command')
                : item.kind === 'workspace'
                  ? $t('app.workspace')
                  : $t('app.document')
            }}
          </em>
        </button>
        <p v-if="commandPaletteSearching" class="command-palette-state">
          {{ $t('app.searching') }}
        </p>
        <p
          v-else-if="!commandPaletteResults.length"
          class="command-palette-state"
        >
          {{ $t('app.noSearchResults') }}
        </p>
      </div>
    </section>
  </div>

  <div
    v-if="filePreview"
    class="modal-backdrop"
    @mousedown.self="$emit('closeFilePreview')"
  >
    <section
      ref="previewSurface"
      class="dialog file-preview-dialog"
      role="dialog"
      aria-modal="true"
      aria-labelledby="file-preview-title"
      tabindex="-1"
    >
      <header>
        <h2 id="file-preview-title">{{ filePreview.path }}</h2>
        <button :aria-label="$t('app.close')" @click="$emit('closeFilePreview')">×</button>
      </header>
      <div>
        <img
          v-if="filePreview.kind === 'image'"
          :src="filePreview.url"
          :alt="filePreview.path"
        />
        <iframe
          v-else-if="filePreview.kind === 'pdf'"
          :src="filePreview.url"
          :title="filePreview.path"
        />
        <audio
          v-else-if="filePreview.kind === 'audio'"
          :src="filePreview.url"
          controls
          autoplay
        />
        <video
          v-else-if="filePreview.kind === 'video'"
          :src="filePreview.url"
          controls
        />
      </div>
    </section>
  </div>
</template>
