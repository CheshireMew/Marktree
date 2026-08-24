<script setup lang="ts">
import { computed, ref, watch } from 'vue'

import { useDialogAccessibility } from '@/composables/useDialogAccessibility'
import type { PendingAndroidShare, WorkspaceDescriptor } from '@/types'

const props = defineProps<{
  share: PendingAndroidShare
  workspaces: WorkspaceDescriptor[]
  selectedRoot: string
  selectedDirectory: string
  directories: string[]
  activeRoot?: string
  activeDocumentPath?: string
  importing?: boolean
}>()

const emit = defineEmits<{
  close: []
  selectRoot: [root: string]
  selectDirectory: [directory: string]
  import: [documentPath?: string]
}>()

const insertIntoDocument = ref(false)
const dialogSurface = ref<HTMLElement>()
const canInsert = computed(
  () =>
    props.share.kind !== 'archive' &&
    Boolean(props.activeDocumentPath) &&
    props.selectedRoot === props.activeRoot,
)

watch(canInsert, (available) => {
  if (!available) insertIntoDocument.value = false
})
useDialogAccessibility(ref(true), dialogSurface, () => emit('close'))
</script>

<template>
  <div class="modal-backdrop">
    <section
      ref="dialogSurface"
      class="dialog android-share-dialog"
      role="dialog"
      aria-modal="true"
      aria-labelledby="android-share-title"
      tabindex="-1"
    >
      <header>
        <div>
          <h2 id="android-share-title">{{ $t('app.importSharedContent') }}</h2>
          <p>{{ share.fileName || share.subject || $t('app.sharedText') }}</p>
        </div>
        <button :aria-label="$t('app.close')" :disabled="importing" @click="emit('close')">×</button>
      </header>

      <template v-if="share.kind === 'archive'">
        <p>{{ $t('app.sharedArchiveCreatesWorkspace') }}</p>
      </template>
      <template v-else>
        <label v-if="workspaces.length">
          <span>{{ $t('app.workspace') }}</span>
          <select :value="selectedRoot" @change="emit('selectRoot', ($event.target as HTMLSelectElement).value)">
            <option v-for="workspace in workspaces" :key="workspace.id" :value="workspace.root">
              {{ workspace.name }}
            </option>
          </select>
        </label>
        <p v-else>{{ $t('app.sharedInboxWillBeCreated') }}</p>
        <label v-if="workspaces.length">
          <span>{{ $t('app.targetDirectory') }}</span>
          <select
            :value="selectedDirectory"
            @change="emit('selectDirectory', ($event.target as HTMLSelectElement).value)"
          >
            <option value="">{{ $t('app.workspaceRoot') }}</option>
            <option v-for="directory in directories" :key="directory" :value="directory">
              {{ directory }}
            </option>
          </select>
        </label>
        <label v-if="canInsert" class="checkbox-label">
          <input v-model="insertIntoDocument" type="checkbox" />
          <span>{{ $t('app.insertIntoCurrentDocument', { path: activeDocumentPath }) }}</span>
        </label>
      </template>

      <footer>
        <button :disabled="importing" @click="emit('close')">{{ $t('app.cancel') }}</button>
        <button
          class="primary"
          :disabled="importing || (share.kind !== 'archive' && workspaces.length > 0 && !selectedRoot)"
          @click="emit('import', insertIntoDocument ? activeDocumentPath : undefined)"
        >
          {{ importing ? $t('app.importing') : $t('app.import') }}
        </button>
      </footer>
    </section>
  </div>
</template>
