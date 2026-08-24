<script setup lang="ts">
import {
  editorPreferences,
  resetEditorPreferences,
} from '@/lib/editor/preferences'
</script>

<template>
  <section class="editor-preferences-panel">
    <header>
      <div>
        <strong>{{ $t('app.editorPreferences') }}</strong>
        <span>{{ $t('app.editorPreferencesHint') }}</span>
      </div>
      <button type="button" @click="resetEditorPreferences">{{ $t('app.reset') }}</button>
    </header>
    <div class="form-grid">
      <label>
        <span>{{ $t('app.editorFont') }}</span>
        <select v-model="editorPreferences.fontFamily">
          <option value="system">{{ $t('app.fontSystem') }}</option>
          <option value="serif">{{ $t('app.fontSerif') }}</option>
          <option value="monospace">{{ $t('app.fontMonospace') }}</option>
        </select>
      </label>
      <label>
        <span>{{ $t('app.editorFontSize') }} · {{ editorPreferences.fontSize }} px</span>
        <input v-model.number="editorPreferences.fontSize" type="range" min="12" max="24" />
      </label>
      <label>
        <span>{{ $t('app.interfaceDensity') }}</span>
        <select v-model="editorPreferences.density">
          <option value="comfortable">{{ $t('app.densityComfortable') }}</option>
          <option value="compact">{{ $t('app.densityCompact') }}</option>
        </select>
      </label>
      <label>
        <span>{{ $t('app.sidebarWidth') }} · {{ editorPreferences.sidebarWidth }} px</span>
        <input
          v-model.number="editorPreferences.sidebarWidth"
          type="range"
          min="220"
          max="440"
          step="10"
        />
      </label>
    </div>
    <label class="checkbox-label">
      <input v-model="editorPreferences.spellcheck" type="checkbox" />
      <span>{{ $t('app.spellcheck') }}</span>
    </label>
  </section>
</template>
