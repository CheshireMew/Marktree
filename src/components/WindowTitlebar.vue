<script setup lang="ts">
import { Copy, Minus, Square, X } from 'lucide-vue-next'
import { onBeforeUnmount, onMounted, ref } from 'vue'

import { isTauri } from '@/lib/api'
import { windowService } from '@/lib/windowService'

defineProps<{
  context?: string
}>()

const emit = defineEmits<{
  error: [reason: unknown]
}>()

const maximized = ref(false)
let unlistenResize: (() => void) | undefined

async function refreshMaximizedState() {
  if (!isTauri()) return
  maximized.value = await windowService.isMaximized()
}

async function runWindowAction(action: 'minimize' | 'maximize' | 'close') {
  if (!isTauri()) return
  try {
    if (action === 'minimize') {
      await windowService.minimize()
      return
    }
    if (action === 'maximize') {
      await windowService.toggleMaximize()
      await refreshMaximizedState()
      return
    }
    await windowService.requestClose()
  } catch (reason) {
    emit('error', reason)
  }
}

onMounted(async () => {
  if (!isTauri()) return
  try {
    await refreshMaximizedState()
    unlistenResize = await windowService.onResized(() => {
      void refreshMaximizedState().catch((reason) => emit('error', reason))
    })
  } catch (reason) {
    emit('error', reason)
  }
})

onBeforeUnmount(() => {
  unlistenResize?.()
})
</script>

<template>
  <header class="window-titlebar" data-tauri-drag-region>
    <div class="window-title" data-tauri-drag-region>
      <img src="/marktree.svg" alt="" draggable="false" data-tauri-drag-region />
      <strong data-tauri-drag-region>{{ $t('app.name') }}</strong>
      <span v-if="context" data-tauri-drag-region>· {{ context }}</span>
    </div>
    <div class="window-controls">
      <button
        type="button"
        :aria-label="$t('app.minimizeWindow')"
        :title="$t('app.minimizeWindow')"
        @click="runWindowAction('minimize')"
      >
        <Minus :size="16" />
      </button>
      <button
        type="button"
        :aria-label="maximized ? $t('app.restoreWindow') : $t('app.maximizeWindow')"
        :title="maximized ? $t('app.restoreWindow') : $t('app.maximizeWindow')"
        @click="runWindowAction('maximize')"
      >
        <Copy v-if="maximized" :size="13" />
        <Square v-else :size="12" />
      </button>
      <button
        type="button"
        class="window-close"
        :aria-label="$t('app.closeWindow')"
        :title="$t('app.closeWindow')"
        @click="runWindowAction('close')"
      >
        <X :size="17" />
      </button>
    </div>
  </header>
</template>
