import { computed, ref } from 'vue'

import { readableError } from '@/lib/errors'
import type {
  UnsavedComparison,
  WorkspaceDiffResult,
  WorkspaceFilePreviewState,
} from '@/types'

export const loading = ref(false)
export const syncing = ref(false)
export const gitBusyAction = ref<string>()
export const message = ref('')
export const error = ref('')
export const diffResult = ref<WorkspaceDiffResult>()
export const diffOpen = ref(false)
export const filePreview = ref<WorkspaceFilePreviewState>()
export const externalComparisons = ref<UnsavedComparison[]>([])
export const externalComparison = computed(() => externalComparisons.value[0])

let loadingOperations = 0

export function setError(reason: unknown) {
  message.value = ''
  error.value = readableError(reason)
}

export function setNotice(value: string) {
  error.value = ''
  message.value = value
}

export function closeDiff() {
  diffOpen.value = false
  diffResult.value = undefined
}

export function closeFilePreview() {
  filePreview.value = undefined
}

export function beginLoading() {
  loadingOperations += 1
  loading.value = true
}

export function endLoading() {
  loadingOperations = Math.max(0, loadingOperations - 1)
  loading.value = loadingOperations > 0
}

export function clearNotice() {
  message.value = ''
  error.value = ''
}
