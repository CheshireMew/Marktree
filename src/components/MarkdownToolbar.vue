<script setup lang="ts">
import {
  Bold,
  Code,
  Heading2,
  Italic,
  Link,
  List,
  ListOrdered,
  ListTodo,
  Paperclip,
  Quote,
  Settings2,
  Sigma,
  Table2,
} from 'lucide-vue-next'

import type { MarkdownSnippet } from '@/lib/editor/snippets'

defineProps<{
  snippets: MarkdownSnippet[]
}>()

defineEmits<{
  action: [action: string]
  snippet: [id: string]
  manageSnippets: []
  chooseExisting: []
}>()
</script>

<template>
  <nav class="markdown-toolbar" :aria-label="$t('app.markdownToolbar')">
    <button :title="$t('app.bold')" :aria-label="$t('app.bold')" @click="$emit('action', 'bold')"><Bold :size="18" /></button>
    <button :title="$t('app.italic')" :aria-label="$t('app.italic')" @click="$emit('action', 'italic')"><Italic :size="18" /></button>
    <button :title="$t('app.heading')" :aria-label="$t('app.heading')" @click="$emit('action', 'heading')"><Heading2 :size="18" /></button>
    <button :title="$t('app.link')" :aria-label="$t('app.link')" @click="$emit('action', 'link')"><Link :size="18" /></button>
    <button :title="$t('app.inlineCode')" :aria-label="$t('app.inlineCode')" @click="$emit('action', 'code')"><Code :size="18" /></button>
    <button :title="$t('app.bulletList')" :aria-label="$t('app.bulletList')" @click="$emit('action', 'bullet')"><List :size="18" /></button>
    <button :title="$t('app.orderedList')" :aria-label="$t('app.orderedList')" @click="$emit('action', 'ordered')"><ListOrdered :size="18" /></button>
    <button :title="$t('app.taskList')" :aria-label="$t('app.taskList')" @click="$emit('action', 'task')"><ListTodo :size="18" /></button>
    <button :title="$t('app.quote')" :aria-label="$t('app.quote')" @click="$emit('action', 'quote')"><Quote :size="18" /></button>
    <button :title="$t('app.table')" :aria-label="$t('app.table')" @click="$emit('action', 'table')"><Table2 :size="18" /></button>
    <button :title="$t('app.formula')" :aria-label="$t('app.formula')" @click="$emit('action', 'formula')"><Sigma :size="18" /></button>
    <button :title="$t('app.chooseExistingFile')" :aria-label="$t('app.chooseExistingFile')" @click="$emit('chooseExisting')">
      <Paperclip :size="18" />
    </button>
    <select
      value=""
      :aria-label="$t('app.insertSnippet')"
      @change="
        $emit('snippet', ($event.target as HTMLSelectElement).value);
        ($event.target as HTMLSelectElement).value = ''
      "
    >
      <option value="" disabled>{{ $t('app.snippets') }}</option>
      <option v-for="snippet in snippets" :key="snippet.id" :value="snippet.id">
        {{ snippet.name }}
      </option>
    </select>
    <button :title="$t('app.manageSnippets')" :aria-label="$t('app.manageSnippets')" @click="$emit('manageSnippets')">
      <Settings2 :size="18" />
    </button>
  </nav>
</template>
