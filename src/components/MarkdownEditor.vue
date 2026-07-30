<script setup lang="ts">
import { defaultKeymap, historyKeymap, indentWithTab } from '@codemirror/commands'
import { markdown } from '@codemirror/lang-markdown'
import { Compartment, EditorState } from '@codemirror/state'
import { EditorView, keymap } from '@codemirror/view'
import { GFM } from '@lezer/markdown'
import { basicSetup } from 'codemirror'
import { onBeforeUnmount, onMounted, ref, watch } from 'vue'

import { editorTheme } from '@/lib/editor/editorTheme'
import { markdownPreviewExtensions } from '@/lib/editor/markdownPreview'
import {
  applyEditorChangesToSource,
  detectLineSeparator,
  editorOffsetFromSource,
  sourceDocument,
  sourceLineSeparator,
  sourceOffsetFromEditor,
} from '@/lib/sourceText'
import type { WorkspaceImageLoader } from '@/types'

const props = defineProps<{
  modelValue: string
  readOnly?: boolean
  dark?: boolean
  root?: string
  path?: string
  markdown?: boolean
  loadWorkspaceImage: WorkspaceImageLoader
}>()

const emit = defineEmits<{
  'update:modelValue': [value: string]
  asset: [file: File, cursor: number]
}>()

const host = ref<HTMLElement>()
const editableCompartment = new Compartment()
const themeCompartment = new Compartment()
const lineSeparatorCompartment = new Compartment()
const previewCompartment = new Compartment()
const languageCompartment = new Compartment()
let editor: EditorView | undefined
let currentLineSeparator = detectLineSeparator(props.modelValue)
let currentSource = props.modelValue
let applyingExternalValue = false

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
    sourceOffsetFromEditor(currentSource, view.state.selection.main.head),
  )
  return true
}

onMounted(() => {
  if (!host.value) return
  editor = new EditorView({
    parent: host.value,
    state: EditorState.create({
      doc: sourceDocument(props.modelValue),
      extensions: [
        basicSetup,
        keymap.of([...defaultKeymap, ...historyKeymap, indentWithTab]),
        languageCompartment.of(
          props.markdown ? markdown({ extensions: [GFM] }) : [],
        ),
        EditorView.lineWrapping,
        previewCompartment.of(
          props.markdown
            ? markdownPreviewExtensions({
                root: props.root,
                path: props.path,
                loadWorkspaceImage: props.loadWorkspaceImage,
              })
            : [],
        ),
        lineSeparatorCompartment.of(sourceLineSeparator(props.modelValue)),
        editableCompartment.of(EditorView.editable.of(!props.readOnly)),
        themeCompartment.of(editorTheme(Boolean(props.dark))),
        EditorView.updateListener.of((update) => {
          if (update.docChanged && !applyingExternalValue) {
            currentSource = applyEditorChangesToSource(
              currentSource,
              update.changes,
              currentLineSeparator,
            )
            emit('update:modelValue', currentSource)
          }
        }),
        EditorView.domEventHandlers({
          paste: handleFiles,
          drop: handleFiles,
        }),
      ],
    }),
  })
})

watch(
  () => props.modelValue,
  (value) => {
    if (!editor) return
    const nextLineSeparator = detectLineSeparator(value)
    if (currentSource === value && currentLineSeparator === nextLineSeparator) {
      return
    }
    currentSource = value
    currentLineSeparator = nextLineSeparator
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
  },
)

watch(
  () => props.readOnly,
  (value) => {
    editor?.dispatch({
      effects: editableCompartment.reconfigure(EditorView.editable.of(!value)),
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
  () => [props.root, props.path, props.markdown] as const,
  ([root, path, isMarkdown]) => {
    editor?.dispatch({
      effects: [
        languageCompartment.reconfigure(
          isMarkdown ? markdown({ extensions: [GFM] }) : [],
        ),
        previewCompartment.reconfigure(
          isMarkdown
            ? markdownPreviewExtensions({
                root,
                path,
                loadWorkspaceImage: props.loadWorkspaceImage,
              })
            : [],
        ),
      ],
    })
  },
)

onBeforeUnmount(() => editor?.destroy())

function focusAt(position: number) {
  if (!editor) return
  const target = editorOffsetFromSource(currentSource, position)
  editor.dispatch({
    selection: { anchor: target },
    effects: EditorView.scrollIntoView(target, { y: 'center' }),
  })
  editor.focus()
}

defineExpose({ focusAt })
</script>

<template>
  <div ref="host" class="markdown-editor" />
</template>
