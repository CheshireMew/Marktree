export * from '@/generated/native'

import type {
  DiffMode,
  DiffResult,
  DocumentContent,
} from '@/generated/native'

export type WorkspaceDiffMode =
  | DiffMode
  | 'worktreeToWorktree'
  | 'unsavedToDisk'

export interface WorkspaceDiffResult extends Omit<DiffResult, 'mode'> {
  mode: WorkspaceDiffMode
}

export interface EditorTab extends DocumentContent {
  root: string
  title: string
  diskContent: string
  revision: number
  savedRevision: number
  dirty: boolean
  saving: boolean
  saveError?: string
}

export interface UnsavedComparison {
  tabKey: string
  root: string
  path: string
  disk: string
  editor: string
  editorRevision: number
  diskMissing: boolean
  externalChange: boolean
}

export type RepositoryImageLoader = (root: string, path: string) => Promise<string>

export interface WorkspaceImagePreview {
  root: string
  path: string
  url: string
}
