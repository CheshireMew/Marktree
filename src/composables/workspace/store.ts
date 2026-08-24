import { reactive, ref } from 'vue'

import { saveWorkspaceSession } from '@/lib/workspaceUiState'
import type {
  BranchDescriptor,
  ConflictRecord,
  DocumentSearchResult,
  EditorTab,
  GitStatusSnapshot,
  PendingGitOperationSummary,
  TrashEntry,
  WorkspaceDescriptor,
  WorkspaceEntry,
  WorktreeSearchResult,
} from '@/types'

export interface WorkspaceSession {
  root: string
  entries: WorkspaceEntry[]
  branches: BranchDescriptor[]
  tabs: EditorTab[]
  activeTabKey?: string
  searchQuery: string
  searchMatches: DocumentSearchResult[]
  crossWorktreeMatches: WorktreeSearchResult[]
  conflicts: ConflictRecord[]
  pendingOperation?: PendingGitOperationSummary
  loadGeneration: number
  searchGeneration: number
  searching: boolean
}

export const workspaces = ref<WorkspaceDescriptor[]>([])
export const activeWorkspaceId = ref<string>()
export const activeWorktreePath = ref<string>()
export const sessions = reactive(new Map<string, WorkspaceSession>())
export const recentFiles = ref<string[]>([])
export const recentFileLimit = ref(Number.POSITIVE_INFINITY)
export const trashEntries = ref<TrashEntry[]>([])

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
      searching: false,
    })
    sessions.set(root, session)
  }
  return session
}

export function retainedEditorTabs() {
  return [...sessions.values()].flatMap((session) => session.tabs)
}

export function addOrReplaceWorkspace(descriptor: WorkspaceDescriptor) {
  const index = workspaces.value.findIndex((item) => item.id === descriptor.id)
  if (index < 0) workspaces.value.push(descriptor)
  else workspaces.value[index] = descriptor
}

export function workspaceForRoot(root: string) {
  return workspaces.value.find(
    (workspace) =>
      workspace.root === root ||
      workspace.git?.worktrees.some((worktree) => worktree.path === root),
  )
}

export function rootHasGitCapability(root: string) {
  return Boolean(workspaceForRoot(root)?.git)
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

export function tabKey(root: string, path: string) {
  return `${root}\n${path}`
}

export function fileName(path: string) {
  return path.split('/').at(-1) ?? path
}

export function persistSessionPaths(session: WorkspaceSession) {
  const active = session.activeTabKey?.split('\n').slice(1).join('\n')
  saveWorkspaceSession(
    session.root,
    session.tabs.map((tab) => tab.path),
    active,
  )
}
