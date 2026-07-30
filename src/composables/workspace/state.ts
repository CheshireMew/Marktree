import { computed, reactive, ref } from 'vue'

import { readableError } from '@/lib/errors'
import type {
  BranchDescriptor,
  ConflictRecord,
  EditorTab,
  GitStatusSnapshot,
  PendingGitOperation,
  TrashEntry,
  UnsavedComparison,
  WorkspaceDiffResult,
  WorkspaceDescriptor,
  WorkspaceEntry,
  WorkspaceImagePreview,
  WorktreeSearchResult,
} from '@/types'

export interface WorkspaceSession {
  root: string
  entries: WorkspaceEntry[]
  branches: BranchDescriptor[]
  tabs: EditorTab[]
  activeTabKey?: string
  searchQuery: string
  searchMatches: string[]
  crossWorktreeMatches: WorktreeSearchResult[]
  conflicts: ConflictRecord[]
  pendingOperation?: PendingGitOperation
  loadGeneration: number
  searchGeneration: number
}

export const workspaces = ref<WorkspaceDescriptor[]>([])
export const activeWorkspaceId = ref<string>()
export const activeWorktreePath = ref<string>()
export const sessions = reactive(new Map<string, WorkspaceSession>())
export const recentFiles = ref<string[]>([])
export const trashEntries = ref<TrashEntry[]>([])
export const loading = ref(false)
export const syncing = ref(false)
export const message = ref('')
export const error = ref('')
export const diffResult = ref<WorkspaceDiffResult>()
export const diffOpen = ref(false)
export const imagePreview = ref<WorkspaceImagePreview>()
export const externalComparisons = ref<UnsavedComparison[]>([])
export const externalComparison = computed(() => externalComparisons.value[0])

let loadingOperations = 0

export const activeWorkspace = computed(() =>
  workspaces.value.find((workspace) => workspace.id === activeWorkspaceId.value),
)

export const activeWorktree = computed(() => {
  const capability = activeWorkspace.value?.git
  if (!capability) return undefined
  return (
    capability.worktrees.find((worktree) => worktree.path === activeWorktreePath.value) ??
    capability.worktrees[0]
  )
})

export const activeRoot = computed(
  () => activeWorktree.value?.path ?? activeWorkspace.value?.root,
)
export const activeStatus = computed<GitStatusSnapshot | undefined>(
  () => activeWorktree.value?.status,
)
export const activeSession = computed(() => {
  const root = activeRoot.value
  return root ? sessions.get(root) : undefined
})
export const entries = computed(() => activeSession.value?.entries ?? [])
export const documents = computed(() =>
  entries.value.filter((entry) => entry.entryType === 'file'),
)
export const branches = computed(() => activeSession.value?.branches ?? [])
export const tabs = computed(() => activeSession.value?.tabs ?? [])
export const activeTabKey = computed<string | undefined>({
  get: () => activeSession.value?.activeTabKey,
  set: (value) => {
    const session = activeSession.value
    if (session) session.activeTabKey = value
  },
})
export const activeTab = computed(() => {
  const session = activeSession.value
  return session?.tabs.find((tab) => tabKey(tab.root, tab.path) === session.activeTabKey)
})
export const searchQuery = computed<string>({
  get: () => activeSession.value?.searchQuery ?? '',
  set: (value) => {
    const session = activeSession.value
    if (session) session.searchQuery = value
  },
})
export const crossWorktreeMatches = computed(
  () => activeSession.value?.crossWorktreeMatches ?? [],
)
export const conflicts = computed(() => activeSession.value?.conflicts ?? [])
export const pendingOperation = computed(() => activeSession.value?.pendingOperation)

export const filteredEntries = computed(() => {
  const session = activeSession.value
  const needle = session?.searchQuery.trim().toLowerCase() ?? ''
  if (!needle) return entries.value
  const contentMatches = new Set(session?.searchMatches ?? [])
  const matchedFiles = entries.value.filter(
    (entry) =>
      entry.entryType === 'file' &&
      (entry.path.toLowerCase().includes(needle) || contentMatches.has(entry.path)),
  )
  const visiblePaths = new Set(matchedFiles.map((entry) => entry.path))
  for (const file of matchedFiles) {
    const parts = file.path.split('/')
    while (parts.length > 1) {
      parts.pop()
      visiblePaths.add(parts.join('/'))
    }
  }
  return entries.value.filter((entry) => visiblePaths.has(entry.path))
})

export const quickOpenDocuments = computed(() => {
  const root = activeRoot.value
  if (!root) return documents.value
  const ranks = new Map(recentFiles.value.map((key, index) => [key, index]))
  return [...documents.value].sort((left, right) => {
    const leftRank = ranks.get(`${root}\n${left.path}`) ?? Number.MAX_SAFE_INTEGER
    const rightRank = ranks.get(`${root}\n${right.path}`) ?? Number.MAX_SAFE_INTEGER
    return leftRank - rightRank || left.path.localeCompare(right.path)
  })
})

export function ensureSession(root: string): WorkspaceSession {
  let session = sessions.get(root)
  if (!session) {
    session = reactive<WorkspaceSession>({
      root,
      entries: [],
      branches: [],
      tabs: [],
      searchQuery: '',
      searchMatches: [],
      crossWorktreeMatches: [],
      conflicts: [],
      pendingOperation: undefined,
      loadGeneration: 0,
      searchGeneration: 0,
    })
    sessions.set(root, session)
  }
  return session
}

export function addOrReplaceWorkspace(descriptor: WorkspaceDescriptor) {
  const index = workspaces.value.findIndex((item) => item.id === descriptor.id)
  if (index < 0) workspaces.value.push(descriptor)
  else workspaces.value[index] = descriptor
}

export function updateWorktreeStatus(root: string, status: GitStatusSnapshot) {
  for (const workspace of workspaces.value) {
    const capability = workspace.git
    if (!capability) continue
    const worktree = capability.worktrees.find((item) => item.path === root)
    if (worktree) {
      worktree.status = status
      worktree.branch = status.branch
    }
    if (workspace.root === root) capability.status = status
  }
}

export function setError(reason: unknown) {
  error.value = readableError(reason)
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

export function tabKey(root: string, path: string) {
  return `${root}\n${path}`
}

export function fileName(path: string) {
  return path.split('/').at(-1) ?? path
}
