<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'

import { useDialogAccessibility } from '@/composables/useDialogAccessibility'
import {
  importMarkdownSnippets,
  markdownSnippets,
  newMarkdownSnippet,
  removeMarkdownSnippet,
  saveMarkdownSnippet,
  serializeMarkdownSnippets,
  type MarkdownSnippet,
} from '@/lib/editor/snippets'

const props = defineProps<{ open: boolean }>()
const emit = defineEmits<{ close: [] }>()
const { t } = useI18n()
const selectedId = ref<string>()
const draft = ref<MarkdownSnippet>(newMarkdownSnippet())
const importInput = ref<HTMLInputElement>()
const importMode = ref<'merge' | 'replace'>('merge')
const transferMessage = ref('')
const pendingAction = ref<(() => void)>()
const pendingMessage = ref('')
const dialogSurface = ref<HTMLElement>()
const canSave = computed(
  () => Boolean(draft.value.name.trim() && draft.value.shortcut.trim() && draft.value.body),
)
const dirty = computed(() => {
  const saved = selectedId.value
    ? markdownSnippets.value.find((snippet) => snippet.id === selectedId.value)
    : undefined
  if (!saved) {
    return Boolean(draft.value.name || draft.value.shortcut || draft.value.body)
  }
  return (
    draft.value.name !== saved.name ||
    draft.value.shortcut !== saved.shortcut ||
    draft.value.body !== saved.body
  )
})
useDialogAccessibility(computed(() => props.open), dialogSurface, requestClose)

watch(
  () => props.open,
  (open) => {
    if (!open) return
    const first = markdownSnippets.value[0]
    pendingAction.value = undefined
    if (first) loadSnippet(first)
    else loadNewSnippet()
  },
)

function selectSnippet(snippet: MarkdownSnippet) {
  if (snippet.id === selectedId.value) return
  requestDiscard(() => loadSnippet(snippet))
}

function loadSnippet(snippet: MarkdownSnippet) {
  selectedId.value = snippet.id
  draft.value = { ...snippet }
}

function createSnippet() {
  requestDiscard(loadNewSnippet)
}

function loadNewSnippet() {
  selectedId.value = undefined
  draft.value = newMarkdownSnippet()
}

function requestDiscard(action: () => void) {
  if (!dirty.value) {
    action()
    return
  }
  pendingMessage.value = t('app.unsavedSnippetConfirm')
  pendingAction.value = action
}

function confirmPending() {
  const action = pendingAction.value
  pendingAction.value = undefined
  action?.()
}

function cancelPending() {
  pendingAction.value = undefined
}

function requestClose() {
  requestDiscard(() => emit('close'))
}

function save() {
  if (!canSave.value) return
  saveMarkdownSnippet(draft.value)
  selectedId.value = draft.value.id
}

function remove() {
  if (!selectedId.value) return
  pendingMessage.value = t('app.deleteSnippetConfirm')
  pendingAction.value = () => {
    removeMarkdownSnippet(selectedId.value!)
    const first = markdownSnippets.value[0]
    if (first) loadSnippet(first)
    else loadNewSnippet()
  }
}

function chooseImport(mode: 'merge' | 'replace') {
  requestDiscard(() => {
    importMode.value = mode
    importInput.value?.click()
  })
}

async function importFile(event: Event) {
  const input = event.target as HTMLInputElement
  const file = input.files?.[0]
  input.value = ''
  if (!file) return
  try {
    const count = importMarkdownSnippets(await file.text(), importMode.value === 'replace')
    transferMessage.value = t('app.snippetsImported', { count })
    const first = markdownSnippets.value[0]
    if (first) selectSnippet(first)
  } catch (error) {
    transferMessage.value = error instanceof Error ? error.message : String(error)
  }
}

async function exportSnippets() {
  const file = new File([serializeMarkdownSnippets()], 'marktree-snippets.json', {
    type: 'application/json',
  })
  try {
    if (navigator.share && navigator.canShare?.({ files: [file] })) {
      await navigator.share({ files: [file], title: t('app.exportSnippets') })
    } else {
      const url = URL.createObjectURL(file)
      const anchor = document.createElement('a')
      anchor.href = url
      anchor.download = file.name
      anchor.click()
      URL.revokeObjectURL(url)
    }
    transferMessage.value = t('app.snippetsExported')
  } catch (error) {
    if (error instanceof DOMException && error.name === 'AbortError') return
    transferMessage.value = error instanceof Error ? error.message : String(error)
  }
}
</script>

<template>
  <div v-if="open" class="modal-backdrop" @mousedown.self="requestClose">
    <section
      ref="dialogSurface"
      class="dialog snippet-dialog"
      role="dialog"
      aria-modal="true"
      aria-labelledby="snippet-dialog-title"
      tabindex="-1"
    >
      <header>
        <div>
          <h2 id="snippet-dialog-title">{{ $t('app.manageSnippets') }}</h2>
          <p>{{ $t('app.snippetHint') }}</p>
        </div>
        <button :aria-label="$t('app.close')" @click="requestClose">×</button>
      </header>
      <div class="snippet-layout">
        <aside>
          <button class="primary" @click="createSnippet">{{ $t('app.newSnippet') }}</button>
          <button
            v-for="snippet in markdownSnippets"
            :key="snippet.id"
            :class="{ active: snippet.id === selectedId }"
            @click="selectSnippet(snippet)"
          >
            <strong>{{ snippet.name }}</strong>
            <small>/{{ snippet.shortcut }}</small>
          </button>
        </aside>
        <div class="snippet-form">
          <label>{{ $t('app.snippetName') }}<input v-model="draft.name" /></label>
          <label>{{ $t('app.snippetShortcut') }}<input v-model="draft.shortcut" /></label>
          <label>
            {{ $t('app.snippetBody') }}
            <textarea v-model="draft.body" rows="11" spellcheck="false" />
          </label>
          <p>{{ $t('app.snippetPlaceholders') }}</p>
        </div>
      </div>
      <section v-if="pendingAction" class="snippet-draft-warning" role="alertdialog" aria-live="assertive">
        <span>{{ pendingMessage }}</span>
        <div>
          <button @click="cancelPending">{{ $t('app.keepEditing') }}</button>
          <button class="danger" @click="confirmPending">{{ $t('app.discardChanges') }}</button>
        </div>
      </section>
      <footer>
        <input
          ref="importInput"
          class="visually-hidden"
          type="file"
          accept="application/json,.json"
          @change="importFile"
        />
        <button v-if="selectedId" class="danger" @click="remove">{{ $t('app.deleteSnippet') }}</button>
        <button @click="chooseImport('merge')">{{ $t('app.importSnippets') }}</button>
        <button @click="chooseImport('replace')">{{ $t('app.replaceSnippets') }}</button>
        <button @click="exportSnippets">{{ $t('app.exportSnippets') }}</button>
        <span>{{ transferMessage }}</span>
        <button @click="requestClose">{{ $t('app.close') }}</button>
        <button class="primary" :disabled="!canSave" @click="save">{{ $t('app.saveSnippet') }}</button>
      </footer>
    </section>
  </div>
</template>
