<script setup lang="ts">
import { autocompletion, type CompletionContext } from '@codemirror/autocomplete'
import { defaultKeymap, historyKeymap, indentWithTab } from '@codemirror/commands'
import { markdown } from '@codemirror/lang-markdown'
import { Compartment, EditorState, type Transaction } from '@codemirror/state'
import { EditorView, keymap, type ViewUpdate } from '@codemirror/view'
import { GFM } from '@lezer/markdown'
import { basicSetup } from 'codemirror'
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'

import MarkdownToolbar from '@/components/MarkdownToolbar.vue'
import SnippetManager from '@/components/SnippetManager.vue'
import { useDialogAccessibility } from '@/composables/useDialogAccessibility'
import { editorTheme } from '@/lib/editor/editorTheme'
import { editorPreferences } from '@/lib/editor/preferences'
import { markdownPreviewExtensions } from '@/lib/editor/markdownPreview'
import {
  markdownSnippets,
  renderMarkdownSnippet,
  type MarkdownSnippet,
} from '@/lib/editor/snippets'
import {
  detectLineSeparator,
  sourceDocument,
  sourceLineSeparator,
  SourceTextBuffer,
} from '@/lib/sourceText'
import type { WorkspaceEntry, WorkspaceImageLoader } from '@/types'

const props = defineProps<{
  modelValue: string
  readOnly?: boolean
  readingMode?: boolean
  dark?: boolean
  root?: string
  path?: string
  markdown?: boolean
  largeDocument?: boolean
  documentCharacterLimit?: number
  linkCandidates?: WorkspaceEntry[]
  loadWorkspaceImage: WorkspaceImageLoader
}>()

const emit = defineEmits<{
  'update:modelValue': [value: string]
  metrics: [value: { words: number | null; lines: number }]
  asset: [file: File, cursor: number]
  limit: []
}>()

const host = ref<HTMLElement>()
const editableCompartment = new Compartment()
const themeCompartment = new Compartment()
const lineSeparatorCompartment = new Compartment()
const previewCompartment = new Compartment()
const languageCompartment = new Compartment()
const autocompleteCompartment = new Compartment()
const contentAttributesCompartment = new Compartment()
const snippetManagerOpen = ref(false)
const filePickerOpen = ref(false)
const filePickerSurface = ref<HTMLElement>()
useDialogAccessibility(filePickerOpen, filePickerSurface, () => {
  filePickerOpen.value = false
})
const filePickerQuery = ref('')
const filteredLinkCandidates = computed(() => {
  const needle = filePickerQuery.value.trim().toLowerCase()
  return (props.linkCandidates ?? []).filter(
    (entry) =>
      entry.entryType === 'file' &&
      entry.path !== props.path &&
      (!needle || entry.path.toLowerCase().includes(needle)),
  )
})
let editor: EditorView | undefined
let currentLineSeparator = detectLineSeparator(props.modelValue)
const sourceBuffer = new SourceTextBuffer(props.modelValue)
let applyingExternalValue = false
let wordCount = props.largeDocument ? null : countWords(props.modelValue)
let lastLimitNotice = 0

function enforceDocumentLimit(transaction: Transaction) {
  const limit = props.documentCharacterLimit
  if (!transaction.docChanged || limit === undefined || transaction.newDoc.length <= limit) {
    return transaction
  }
  const now = Date.now()
  if (now - lastLimitNotice >= 1_000) {
    lastLimitNotice = now
    queueMicrotask(() => emit('limit'))
  }
  return []
}

function countWords(content: string) {
  return content.match(
    /[\p{Script=Han}\p{Script=Hiragana}\p{Script=Katakana}]|[\p{L}\p{N}]+/gu,
  )?.length ?? 0
}

function emitMetrics(view: EditorView) {
  emit('metrics', { words: wordCount, lines: view.state.doc.lines })
}

function updateWordCount(update: ViewUpdate) {
  if (wordCount === null || !update.docChanged) return
  const removedRanges: Array<[number, number]> = []
  const insertedRanges: Array<[number, number]> = []
  update.changes.iterChangedRanges((fromA, toA, fromB, toB) => {
    removedRanges.push([
      update.startState.doc.lineAt(fromA).from,
      update.startState.doc.lineAt(toA).to,
    ])
    insertedRanges.push([
      update.state.doc.lineAt(fromB).from,
      update.state.doc.lineAt(toB).to,
    ])
  })
  const countRanges = (
    document: ViewUpdate['state']['doc'],
    ranges: Array<[number, number]>,
  ) => {
    let total = 0
    let current: [number, number] | undefined
    for (const range of ranges.sort((left, right) => left[0] - right[0])) {
      if (current && range[0] <= current[1]) current[1] = Math.max(current[1], range[1])
      else {
        if (current) total += countWords(document.sliceString(current[0], current[1]))
        current = [...range]
      }
    }
    if (current) total += countWords(document.sliceString(current[0], current[1]))
    return total
  }
  wordCount +=
    countRanges(update.state.doc, insertedRanges) -
    countRanges(update.startState.doc, removedRanges)
}

function handleFiles(event: ClipboardEvent | DragEvent, view: EditorView) {
  const files =
    event instanceof ClipboardEvent
      ? [...(event.clipboardData?.files ?? [])]
      : [...(event.dataTransfer?.files ?? [])]
  const image = files.find((file) => file.type.startsWith('image/'))
  if (!image || !props.markdown) return false
  event.preventDefault()
  emit(
    'asset',
    image,
    sourceBuffer.sourceOffset(view.state.selection.main.head),
  )
  return true
}

function snippetCompletions(context: CompletionContext) {
  if (!props.markdown || !markdownSnippets.value.length) return null
  const word = context.matchBefore(/\/[\w-]*/)
  if (!word || (word.from === word.to && !context.explicit)) return null
  return {
    from: word.from,
    options: markdownSnippets.value.map((snippet) => ({
      label: `/${snippet.shortcut}`,
      detail: snippet.name,
      type: 'text',
      apply(view: EditorView, _completion: unknown, from: number, to: number) {
        applySnippet(view, snippet, from, to, '')
      },
    })),
  }
}

function linkCompletions(context: CompletionContext) {
  if (!props.markdown || !props.path || !props.linkCandidates?.length) return null
  const destination = context.matchBefore(/\]\((?:<)?[^)\n>]*/)
  if (!destination || (!context.explicit && destination.from === destination.to)) return null
  const angled = destination.text.startsWith('](<')
  const from = destination.from + (angled ? 3 : 2)
  return {
    from,
    options: props.linkCandidates
      .filter((entry) => entry.entryType === 'file' && entry.path !== props.path)
      .map((entry) => {
        const relative = relativeWorkspacePath(props.path!, entry.path)
        return {
          label: relative,
          detail: entry.fileKind ?? undefined,
          type: entry.fileKind === 'image' ? 'image' : 'file',
          apply: angled ? `${relative}>` : markdownDestination(relative),
        }
      }),
  }
}

onMounted(() => {
  if (!host.value) return
  editor = new EditorView({
    parent: host.value,
    state: EditorState.create({
      doc: sourceDocument(props.modelValue),
      extensions: [
        basicSetup,
        EditorState.transactionFilter.of(enforceDocumentLimit),
        keymap.of([...defaultKeymap, ...historyKeymap, indentWithTab]),
        languageCompartment.of(
          props.markdown ? markdown({ extensions: [GFM] }) : [],
        ),
        autocompleteCompartment.of(
          props.markdown && !props.largeDocument
            ? autocompletion({ override: [snippetCompletions, linkCompletions] })
            : [],
        ),
        EditorView.lineWrapping,
        contentAttributesCompartment.of(
          EditorView.contentAttributes.of({
            spellcheck: editorPreferences.spellcheck ? 'true' : 'false',
          }),
        ),
        previewCompartment.of(
          props.markdown && !props.largeDocument
            ? markdownPreviewExtensions({
                root: props.root,
                path: props.path,
                loadWorkspaceImage: props.loadWorkspaceImage,
              }, Boolean(props.readingMode))
            : [],
        ),
        lineSeparatorCompartment.of(sourceLineSeparator(props.modelValue)),
        editableCompartment.of(
          EditorView.editable.of(!props.readOnly && !props.readingMode),
        ),
        themeCompartment.of(editorTheme(Boolean(props.dark))),
        EditorView.updateListener.of((update) => {
          if (update.docChanged && !applyingExternalValue) {
            updateWordCount(update)
            const source = sourceBuffer.apply(
              update.changes,
              currentLineSeparator,
            )
            emit('update:modelValue', source)
            emitMetrics(update.view)
          }
        }),
        EditorView.domEventHandlers({
          paste: handleFiles,
          drop: handleFiles,
        }),
      ],
    }),
  })
  emitMetrics(editor)
})

watch(
  () => props.largeDocument,
  (largeDocument) => {
    wordCount = largeDocument ? null : countWords(sourceBuffer.source)
    if (editor) emitMetrics(editor)
  },
)

watch(
  () => props.modelValue,
  (value) => {
    if (!editor) return
    const nextLineSeparator = detectLineSeparator(value)
    if (sourceBuffer.source === value && currentLineSeparator === nextLineSeparator) {
      return
    }
    sourceBuffer.replace(value)
    currentLineSeparator = nextLineSeparator
    wordCount = props.largeDocument ? null : countWords(value)
    applyingExternalValue = true
    editor.dispatch({
      changes: {
        from: 0,
        to: editor.state.doc.length,
        insert: sourceDocument(value),
      },
      effects: lineSeparatorCompartment.reconfigure(sourceLineSeparator(value)),
    })
    applyingExternalValue = false
    emitMetrics(editor)
  },
)

watch(
  () => [props.readOnly, props.readingMode] as const,
  ([readOnly, readingMode]) => {
    editor?.dispatch({
      effects: editableCompartment.reconfigure(
        EditorView.editable.of(!readOnly && !readingMode),
      ),
    })
  },
)

watch(
  () => props.dark,
  (value) => {
    editor?.dispatch({
      effects: themeCompartment.reconfigure(editorTheme(Boolean(value))),
    })
  },
)

watch(
  () => editorPreferences.spellcheck,
  (spellcheck) => {
    editor?.dispatch({
      effects: contentAttributesCompartment.reconfigure(
        EditorView.contentAttributes.of({ spellcheck: spellcheck ? 'true' : 'false' }),
      ),
    })
  },
)

watch(
  () => [props.root, props.path, props.markdown, props.linkCandidates, props.readingMode, props.largeDocument] as const,
  ([root, path, isMarkdown, _linkCandidates, readingMode, largeDocument]) => {
    editor?.dispatch({
      effects: [
        languageCompartment.reconfigure(
          isMarkdown ? markdown({ extensions: [GFM] }) : [],
        ),
        autocompleteCompartment.reconfigure(
          isMarkdown && !largeDocument
            ? autocompletion({ override: [snippetCompletions, linkCompletions] })
            : [],
        ),
        previewCompartment.reconfigure(
          isMarkdown && !largeDocument
            ? markdownPreviewExtensions({
                root,
                path,
                loadWorkspaceImage: props.loadWorkspaceImage,
              }, Boolean(readingMode))
            : [],
        ),
      ],
    })
  },
)

onBeforeUnmount(() => editor?.destroy())

function focusAt(position: number) {
  if (!editor) return
  const target = sourceBuffer.editorOffset(position)
  editor.dispatch({
    selection: { anchor: target },
    effects: EditorView.scrollIntoView(target, { y: 'center' }),
  })
  editor.focus()
}

function focusAtLine(lineNumber: number, columnNumber = 1) {
  if (!editor) return
  const line = editor.state.doc.line(
    Math.max(1, Math.min(lineNumber, editor.state.doc.lines)),
  )
  const target = Math.min(line.to, line.from + Math.max(0, columnNumber - 1))
  editor.dispatch({
    selection: { anchor: target },
    effects: EditorView.scrollIntoView(target, { y: 'center' }),
  })
  editor.focus()
}

function replaceSelection(text: string, cursor: number, selectLength = 0) {
  if (!editor || props.readOnly || props.readingMode) return
  const selection = editor.state.selection.main
  editor.dispatch({
    changes: { from: selection.from, to: selection.to, insert: text },
    selection: {
      anchor: selection.from + cursor,
      head: selection.from + cursor + selectLength,
    },
  })
  editor.focus()
}

function wrapSelection(before: string, after: string, placeholder: string) {
  if (!editor) return
  const selection = editor.state.selection.main
  const selected = editor.state.sliceDoc(selection.from, selection.to)
  const content = selected || placeholder
  const text = `${before}${content}${after}`
  replaceSelection(
    text,
    before.length + (selected ? content.length : 0),
    selected ? 0 : content.length,
  )
}

function prefixSelectedLines(prefix: string, replaceHeading = false) {
  if (!editor || props.readOnly || props.readingMode) return
  const selection = editor.state.selection.main
  const first = editor.state.doc.lineAt(selection.from)
  const last = editor.state.doc.lineAt(selection.to)
  const source = editor.state.sliceDoc(first.from, last.to)
  const lines = source.split('\n')
  const text = lines
    .map((line, index) => {
      const value = replaceHeading ? line.replace(/^#{1,6}\s+/, '') : line
      return prefix === '1. ' ? `${index + 1}. ${value}` : `${prefix}${value}`
    })
    .join('\n')
  editor.dispatch({
    changes: { from: first.from, to: last.to, insert: text },
    selection: { anchor: first.from, head: first.from + text.length },
  })
  editor.focus()
}

function applyMarkdownAction(action: string) {
  if (!editor || props.readOnly || props.readingMode || !props.markdown) return
  if (action === 'bold') wrapSelection('**', '**', 'text')
  else if (action === 'italic') wrapSelection('*', '*', 'text')
  else if (action === 'code') wrapSelection('`', '`', 'code')
  else if (action === 'link') wrapSelection('[', '](https://)', 'link text')
  else if (action === 'heading') prefixSelectedLines('## ', true)
  else if (action === 'bullet') prefixSelectedLines('- ')
  else if (action === 'ordered') prefixSelectedLines('1. ')
  else if (action === 'task') prefixSelectedLines('- [ ] ')
  else if (action === 'quote') prefixSelectedLines('> ')
  else if (action === 'table') {
    const selection = editor.state.selection.main
    const selected = editor.state.sliceDoc(selection.from, selection.to) || 'Value'
    const before = '| Column 1 | Column 2 |\n| --- | --- |\n| '
    replaceSelection(`${before}${selected} |  |`, before.length, selected === 'Value' ? selected.length : 0)
  } else if (action === 'formula') {
    const selection = editor.state.selection.main
    const selected = editor.state.sliceDoc(selection.from, selection.to)
    if (selected.includes('\n')) wrapSelection('$$\n', '\n$$', 'formula')
    else wrapSelection('$', '$', 'formula')
  }
}

function relativeWorkspacePath(documentPath: string, targetPath: string) {
  const source = documentPath.split('/').slice(0, -1)
  const target = targetPath.split('/')
  while (source.length && target.length && source[0] === target[0]) {
    source.shift()
    target.shift()
  }
  return [...source.map(() => '..'), ...target].join('/') || '.'
}

function markdownDestination(path: string) {
  return /[\s()<>]/.test(path) ? `<${path}>` : path
}

function insertWorkspaceLink(entry: WorkspaceEntry) {
  if (!props.path) return
  const destination = markdownDestination(relativeWorkspacePath(props.path, entry.path))
  const name = entry.name.replace(/\.[^.]+$/, '')
  insertText(
    entry.fileKind === 'image'
      ? `![${name}](${destination})`
      : `[${entry.name}](${destination})`,
  )
  filePickerOpen.value = false
  filePickerQuery.value = ''
}

function applySnippet(
  view: EditorView,
  snippet: MarkdownSnippet,
  from: number,
  to: number,
  selection: string,
) {
  const rendered = renderMarkdownSnippet(snippet.body, selection)
  view.dispatch({
    changes: { from, to, insert: rendered.text },
    selection: { anchor: from + rendered.cursor },
  })
  view.focus()
}

function insertSnippet(id: string) {
  if (!editor || props.readOnly || props.readingMode) return
  const snippet = markdownSnippets.value.find((item) => item.id === id)
  if (!snippet) return
  const selection = editor.state.selection.main
  applySnippet(
    editor,
    snippet,
    selection.from,
    selection.to,
    editor.state.sliceDoc(selection.from, selection.to),
  )
}

function insertText(text: string) {
  if (!editor || props.readOnly || props.readingMode) return
  replaceSelection(text, text.length)
}

function openSnippetManager() {
  snippetManagerOpen.value = true
}

defineExpose({ focusAt, focusAtLine, insertText, openSnippetManager })
</script>

<template>
  <section class="markdown-editor-shell" :class="{ 'reading-mode': props.readingMode }">
    <MarkdownToolbar
      v-if="props.markdown && !props.readOnly && !props.readingMode"
      :snippets="markdownSnippets"
      @action="applyMarkdownAction"
      @snippet="insertSnippet"
      @manage-snippets="openSnippetManager"
      @choose-existing="filePickerOpen = true"
    />
    <div ref="host" class="markdown-editor" />
    <SnippetManager :open="snippetManagerOpen" @close="snippetManagerOpen = false" />
    <div v-if="filePickerOpen" class="modal-backdrop" @mousedown.self="filePickerOpen = false">
      <section
        ref="filePickerSurface"
        class="dialog workspace-link-picker"
        role="dialog"
        aria-modal="true"
        aria-labelledby="workspace-link-picker-title"
        tabindex="-1"
      >
        <header>
          <div>
            <h2 id="workspace-link-picker-title">{{ $t('app.chooseExistingFile') }}</h2>
            <p>{{ $t('app.chooseExistingFileHint') }}</p>
          </div>
          <button :aria-label="$t('app.close')" @click="filePickerOpen = false">×</button>
        </header>
        <input v-model="filePickerQuery" autofocus :placeholder="$t('app.search')" />
        <div class="workspace-link-results">
          <button
            v-for="entry in filteredLinkCandidates"
            :key="entry.path"
            @click="insertWorkspaceLink(entry)"
          >
            <strong>{{ entry.name }}</strong><small>{{ entry.path }}</small>
          </button>
          <p v-if="!filteredLinkCandidates.length">{{ $t('app.noSearchResults') }}</p>
        </div>
      </section>
    </div>
  </section>
</template>
