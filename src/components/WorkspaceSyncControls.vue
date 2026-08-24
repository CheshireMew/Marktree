<script setup lang="ts">
import { RefreshCw, X } from 'lucide-vue-next'

defineProps<{
  pending: boolean
  syncing: boolean
  compact?: boolean
}>()

defineEmits<{
  abort: []
  sync: []
}>()
</script>

<template>
  <template v-if="compact">
    <button v-if="pending" :disabled="syncing" @click="$emit('abort')">
      {{ $t('app.abortGitOperation') }}
    </button>
    <template v-if="pending"> · </template>
    <button :disabled="syncing" @click="$emit('sync')">
      {{ syncing ? $t('app.syncing') : $t('app.sync') }}
    </button>
  </template>
  <template v-else>
    <button v-if="pending" :disabled="syncing" @click="$emit('abort')">
      <X :size="16" /> {{ $t('app.abortGitOperation') }}
    </button>
    <button
      class="primary sync-button"
      :disabled="syncing"
      @click="$emit('sync')"
    >
      <RefreshCw :size="16" :class="{ spinning: syncing }" />
      {{ syncing ? $t('app.syncing') : $t('app.sync') }}
    </button>
  </template>
</template>
