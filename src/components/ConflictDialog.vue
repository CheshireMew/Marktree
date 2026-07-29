<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import type { ConflictRecord } from '@/types'
import ThreeWayConflictResolver from './ThreeWayConflictResolver.vue'

const props = defineProps<{
  conflicts: ConflictRecord[]
  nativeAndroid: boolean
  syncing: boolean
}>()

const emit = defineEmits<{
  abort: []
  resolveChoice: [conflict: ConflictRecord, choice: 'local' | 'remote']
  resolveContent: [conflict: ConflictRecord, content: string]
}>()

const { t } = useI18n()
const conflict = computed(() => props.conflicts[0])
const canMergeText = computed(
  () =>
    !props.nativeAndroid &&
    conflict.value?.kind === 'text' &&
    conflict.value.localExists &&
    conflict.value.remoteExists,
)

function sideText(side: 'local' | 'remote') {
  const current = conflict.value
  if (!current) return ''
  const exists = side === 'local' ? current.localExists : current.remoteExists
  if (!exists) return t('app.deletedVersion')
  const content = side === 'local' ? current.local : current.remote
  return content ?? t('app.binaryConflictVersion')
}
</script>

<template>
  <div v-if="conflict" class="modal-backdrop">
    <section class="dialog comparison-dialog conflict-dialog">
      <header>
        <div>
          <h2>{{ $t('app.conflictTitle') }}</h2>
          <p>{{ conflict.path }} · {{ $t('app.recoveryHint') }}</p>
        </div>
        <button :disabled="syncing" @click="emit('abort')">
          {{ $t('app.abortGitOperation') }}
        </button>
      </header>
      <ThreeWayConflictResolver
        v-if="canMergeText"
        :conflict="conflict"
        @resolve="emit('resolveContent', conflict, $event)"
      />
      <div v-else class="comparison-grid">
        <article>
          <h3>{{ $t('app.keepLocal') }}</h3>
          <pre>{{ sideText('local') }}</pre>
        </article>
        <article>
          <h3>{{ $t('app.keepRemote') }}</h3>
          <pre>{{ sideText('remote') }}</pre>
        </article>
      </div>
      <footer v-if="!canMergeText">
        <button @click="emit('resolveChoice', conflict, 'remote')">
          {{ $t('app.keepRemote') }}
        </button>
        <button class="primary" @click="emit('resolveChoice', conflict, 'local')">
          {{ $t('app.keepLocal') }}
        </button>
      </footer>
    </section>
  </div>
</template>
