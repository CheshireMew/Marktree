<script setup lang="ts">
import { ChevronDown, Columns2, Rows3, X } from 'lucide-vue-next'
import { diffWords } from 'diff'
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'

import type { DiffLine, DiffMode, WorkspaceDiffResult } from '@/types'

const props = defineProps<{
  result: WorkspaceDiffResult
}>()

defineEmits<{
  close: []
  mode: [mode: DiffMode]
}>()

const layout = ref<'inline' | 'split'>('inline')
const activeChange = ref(-1)
const { t } = useI18n()
const modes = computed<Array<{ value: DiffMode; label: string }>>(() => [
  { value: 'worktreeToIndex', label: t('app.diffWorktreeIndex') },
  { value: 'indexToHead', label: t('app.diffIndexHead') },
  { value: 'worktreeToHead', label: t('app.diffWorktreeHead') },
  { value: 'localToUpstream', label: t('app.diffLocalRemote') },
])

const hunkCount = computed(() =>
  props.result.files.reduce((count, file) => count + file.hunks.length, 0),
)

function oldText(line: DiffLine) {
  return line.kind === 'addition' ? '' : line.content
}

function newText(line: DiffLine) {
  return line.kind === 'deletion' ? '' : line.content
}

function wordSegments(lines: DiffLine[], index: number) {
  const line = lines[index]
  if (!line) return [{ value: '', changed: false }]
  if (line.kind === 'deletion' && lines[index + 1]?.kind === 'addition') {
    return diffWords(line.content, lines[index + 1]?.content ?? '')
      .filter((part) => !part.added)
      .map((part) => ({ value: part.value, changed: Boolean(part.removed) }))
  }
  if (line.kind === 'addition' && lines[index - 1]?.kind === 'deletion') {
    return diffWords(lines[index - 1]?.content ?? '', line.content)
      .filter((part) => !part.removed)
      .map((part) => ({ value: part.value, changed: Boolean(part.added) }))
  }
  return [{ value: line.content, changed: false }]
}

function hunkId(fileIndex: number, hunkIndex: number) {
  return `marktree-diff-${fileIndex}-${hunkIndex}`
}

function jumpNext() {
  if (!hunkCount.value) return
  activeChange.value = (activeChange.value + 1) % hunkCount.value
  let cursor = 0
  for (let fileIndex = 0; fileIndex < props.result.files.length; fileIndex += 1) {
    const file = props.result.files[fileIndex]
    if (!file) continue
    for (let hunkIndex = 0; hunkIndex < file.hunks.length; hunkIndex += 1) {
      if (cursor === activeChange.value) {
        document.getElementById(hunkId(fileIndex, hunkIndex))?.scrollIntoView({
          behavior: 'smooth',
          block: 'start',
        })
        return
      }
      cursor += 1
    }
  }
}
</script>

<template>
  <section class="diff-panel">
    <header class="panel-header diff-header">
      <div>
        <h3>{{ $t('app.diff') }}</h3>
        <span>
          <b class="addition">+{{ result.insertions }}</b>
          <b class="deletion">−{{ result.deletions }}</b>
          · {{ $t('app.changeCount', { count: hunkCount }) }}
        </span>
      </div>
      <div class="diff-controls">
        <select
          :value="result.mode"
          @change="$emit('mode', ($event.target as HTMLSelectElement).value as DiffMode)"
        >
          <option v-if="result.mode === 'worktreeToWorktree'" value="worktreeToWorktree" disabled>
            {{ $t('app.diffWorktreeWorktree') }}
          </option>
          <option v-if="result.mode === 'unsavedToDisk'" value="unsavedToDisk" disabled>
            {{ $t('app.diffUnsavedDisk') }}
          </option>
          <option v-for="mode in modes" :key="mode.value" :value="mode.value">
            {{ mode.label }}
          </option>
        </select>
        <button :title="$t('app.nextChange')" @click="jumpNext"><ChevronDown :size="15" /></button>
        <button :class="{ active: layout === 'inline' }" @click="layout = 'inline'">
          <Rows3 :size="15" />
        </button>
        <button :class="{ active: layout === 'split' }" @click="layout = 'split'">
          <Columns2 :size="15" />
        </button>
        <button @click="$emit('close')"><X :size="16" /></button>
      </div>
    </header>

    <div class="diff-content" :class="layout">
      <article v-for="(file, fileIndex) in result.files" :key="file.path" class="diff-file">
        <header>
          <span>{{ file.path }}</span>
          <i>{{ file.status }}</i>
        </header>
        <div v-if="file.binary" class="binary-change">{{ $t('app.binaryChanged') }}</div>
        <section
          v-for="(hunk, hunkIndex) in file.hunks"
          :id="hunkId(fileIndex, hunkIndex)"
          :key="hunkIndex"
          class="diff-hunk"
        >
          <div class="hunk-header">{{ hunk.header }}</div>
          <template v-if="layout === 'inline'">
            <div v-for="(line, lineIndex) in hunk.lines" :key="lineIndex" class="diff-line" :class="line.kind">
              <span class="line-number">{{ line.oldLine ?? '' }}</span>
              <span class="line-number">{{ line.newLine ?? '' }}</span>
              <code><i>{{ line.kind === 'addition' ? '+' : line.kind === 'deletion' ? '−' : ' ' }}</i><span
                v-for="(segment, segmentIndex) in wordSegments(hunk.lines, lineIndex)"
                :key="segmentIndex"
                :class="{ 'word-change': segment.changed }"
              >{{ segment.value }}</span></code>
            </div>
          </template>
          <template v-else>
            <div v-for="(line, lineIndex) in hunk.lines" :key="lineIndex" class="split-line">
              <div :class="{ deletion: line.kind === 'deletion' }">
                <span>{{ line.oldLine ?? '' }}</span><code>{{ oldText(line) }}</code>
              </div>
              <div :class="{ addition: line.kind === 'addition' }">
                <span>{{ line.newLine ?? '' }}</span><code>{{ newText(line) }}</code>
              </div>
            </div>
          </template>
        </section>
      </article>
      <div v-if="!result.files.length" class="empty-diff">{{ $t('app.clean') }}</div>
    </div>
  </section>
</template>
