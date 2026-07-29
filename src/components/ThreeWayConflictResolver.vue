<script setup lang="ts">
import { computed, ref, watch } from 'vue'

import { mergeThreeWay, resolvedMergeContent } from '@/lib/threeWayMerge'
import type { ConflictRecord } from '@/types'

const props = defineProps<{
  conflict: ConflictRecord
}>()

const emit = defineEmits<{
  resolve: [content: string]
}>()

const segments = ref(buildSegments())
const finalContent = ref(resolvedMergeContent(segments.value))
const conflictingSegments = computed(() => segments.value.filter((segment) => segment.conflicting))

watch(
  () => props.conflict.recoveryId,
  () => {
    segments.value = buildSegments()
    finalContent.value = resolvedMergeContent(segments.value)
  },
)

function buildSegments() {
  return mergeThreeWay(
    props.conflict.ancestor ?? '',
    props.conflict.local ?? '',
    props.conflict.remote ?? '',
  ).segments
}

function choose(segmentId: string, content: string) {
  const segment = segments.value.find((candidate) => candidate.id === segmentId)
  if (!segment) return
  segment.content = content
  finalContent.value = resolvedMergeContent(segments.value)
}
</script>

<template>
  <div class="three-way-resolver">
    <div class="merge-summary">
      {{ $t('app.conflictSegments', { count: conflictingSegments.length }) }}
    </div>
    <div class="merge-segments">
      <article v-for="(segment, index) in conflictingSegments" :key="segment.id">
        <header>{{ $t('app.conflictSegment', { index: index + 1 }) }}</header>
        <div class="merge-candidates">
          <button @click="choose(segment.id, segment.base)">
            <span>{{ $t('app.mergeBase') }}</span><pre>{{ segment.base }}</pre>
          </button>
          <button @click="choose(segment.id, segment.local)">
            <span>{{ $t('app.keepLocal') }}</span><pre>{{ segment.local }}</pre>
          </button>
          <button @click="choose(segment.id, segment.remote)">
            <span>{{ $t('app.keepRemote') }}</span><pre>{{ segment.remote }}</pre>
          </button>
        </div>
        <label>
          <span>{{ $t('app.mergedSegment') }}</span>
          <textarea v-model="segment.content" @input="finalContent = resolvedMergeContent(segments)" />
        </label>
      </article>
    </div>
    <label class="merge-final">
      <span>{{ $t('app.finalMergedContent') }}</span>
      <textarea v-model="finalContent" />
    </label>
    <footer>
      <button class="primary" @click="emit('resolve', finalContent)">
        {{ $t('app.applyMergedContent') }}
      </button>
    </footer>
  </div>
</template>
