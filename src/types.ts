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
  truncated: boolean
  omittedLines: number
}

export interface EditorTab extends DocumentContent {
  root: string
  title: string
  /** Kept only for ordinary documents; large documents read the disk baseline on demand. */
  diskContent?: string
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

export type WorkspaceImageLoader = (root: string, path: string) => Promise<string>

export interface WorkspaceFilePreviewState {
  root: string
  path: string
  url: string
  kind: import('@/generated/native').DocumentKind
  mediaType: string
}

export type CommandPaletteAction =
  | 'newDocument'
  | 'newFolder'
  | 'refresh'
  | 'settings'
  | 'sync'
  | 'snippets'
  | 'addWorkspace'

export type CommandPaletteItem =
  | {
      key: string
      kind: 'command'
      action: CommandPaletteAction
      title: string
      detail: string
    }
  | {
      key: string
      kind: 'document'
      title: string
      detail: string
      result: import('@/generated/native').WorktreeSearchResult
    }
  | {
      key: string
      kind: 'workspace'
      title: string
      detail: string
      workspaceId: string
      worktreeRoot?: string
    }
