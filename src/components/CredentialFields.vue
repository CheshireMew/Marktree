<script setup lang="ts">
defineProps<{ disabled?: boolean }>()
const username = defineModel<string>('username', { required: true })
const token = defineModel<string>('token', { required: true })
</script>

<template>
  <label>
    <span>{{ $t('app.username') }}</span>
    <input
      v-model="username"
      :disabled="disabled"
      autocomplete="username"
      placeholder="username"
    />
  </label>
  <label>
    <span>{{ $t('app.personalAccessToken') }}</span>
    <input
      v-model="token"
      :disabled="disabled"
      type="password"
      autocomplete="current-password"
    />
  </label>
</template>
