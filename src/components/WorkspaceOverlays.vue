<script setup lang="ts">
import { FileText, Search } from 'lucide-vue-next'

import type {
  DocumentDescriptor,
  UnsavedComparison,
  WorkspaceImagePreview,
} from '@/types'

defineProps<{
  externalComparison?: UnsavedComparison
  quickOpen: boolean
  quickOpenQuery: string
  quickOpenResults: DocumentDescriptor[]
  imagePreview?: WorkspaceImagePreview
}>()

defineEmits<{
  chooseExternalVersion: [choice: 'disk' | 'editor']
  closeQuickOpen: []
  updateQuickOpenQuery: [value: string]
  chooseQuickOpen: [path: string]
  closeImagePreview: []
}>()
</script>

<template>
  <div v-if="externalComparison" class="modal-backdrop">
    <section class="dialog comparison-dialog">
      <header>
        <div>
          <h2>{{ $t('app.externalTitle') }}</h2>
          <p>{{ $t('app.externalBody') }}</p>
        </div>
      </header>
      <div class="comparison-grid">
        <article>
          <h3>{{ $t('app.disk') }}</h3>
          <pre>{{ externalComparison.disk }}</pre>
        </article>
        <article>
          <h3>{{ $t('app.editor') }}</h3>
          <pre>{{ externalComparison.editor }}</pre>
        </article>
      </div>
      <footer>
        <button @click="$emit('chooseExternalVersion', 'disk')">
          {{ $t('app.useDisk') }}
        </button>
        <button class="primary" @click="$emit('chooseExternalVersion', 'editor')">
          {{ $t('app.keepEditor') }}
        </button>
      </footer>
    </section>
  </div>

  <div
    v-if="quickOpen"
    class="modal-backdrop"
    @mousedown.self="$emit('closeQuickOpen')"
  >
    <section class="quick-open-dialog">
      <label>
        <Search :size="18" />
        <input
          :value="quickOpenQuery"
          autofocus
          :placeholder="$t('app.quickOpen')"
          @input="
            $emit(
              'updateQuickOpenQuery',
              ($event.target as HTMLInputElement).value,
            )
          "
        />
        <kbd>Esc</kbd>
      </label>
      <div>
        <button
          v-for="document in quickOpenResults"
          :key="document.path"
          @click="$emit('chooseQuickOpen', document.path)"
        >
          <FileText :size="16" />
          <span>
            <strong>{{ document.name }}</strong>
            <small>{{ document.path }}</small>
          </span>
        </button>
      </div>
    </section>
  </div>

  <div
    v-if="imagePreview"
    class="modal-backdrop"
    @mousedown.self="$emit('closeImagePreview')"
  >
    <section class="dialog image-preview-dialog">
      <header>
        <h2>{{ imagePreview.path }}</h2>
        <button @click="$emit('closeImagePreview')">×</button>
      </header>
      <div>
        <img :src="imagePreview.url" :alt="imagePreview.path" />
      </div>
    </section>
  </div>
</template>
