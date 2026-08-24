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
const finalManuallyEdited = ref(false)
const conflictingSegments = computed(() => segments.value.filter((segment) => segment.conflicting))

watch(
  () => props.conflict.recoveryId,
  () => {
    segments.value = buildSegments()
    finalContent.value = resolvedMergeContent(segments.value)
    finalManuallyEdited.value = false
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
  updateGeneratedFinal()
}

function updateGeneratedFinal() {
  if (!finalManuallyEdited.value) {
    finalContent.value = resolvedMergeContent(segments.value)
  }
}

function regenerateFinal() {
  finalContent.value = resolvedMergeContent(segments.value)
  finalManuallyEdited.value = false
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
          <textarea v-model="segment.content" @input="updateGeneratedFinal" />
        </label>
      </article>
    </div>
    <label class="merge-final">
      <span>{{ $t('app.finalMergedContent') }}</span>
      <textarea v-model="finalContent" @input="finalManuallyEdited = true" />
    </label>
    <div v-if="finalManuallyEdited" class="merge-manual-state" role="status">
      <span>{{ $t('app.manualMergePreserved') }}</span>
      <button @click="regenerateFinal">{{ $t('app.regenerateMergedContent') }}</button>
    </div>
    <footer>
      <button class="primary" @click="emit('resolve', finalContent)">
        {{ $t('app.applyMergedContent') }}
      </button>
    </footer>
  </div>
</template>
