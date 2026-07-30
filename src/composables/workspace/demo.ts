import { reactive } from 'vue'

import type { GitStatusSnapshot, WorkspaceDescriptor } from '@/types'

import {
  activeWorkspaceId,
  activeWorktreePath,
  ensureSession,
  fileName,
  workspaces,
} from './state'
import { openDocument } from './documents'
import { demoContents } from './demoData'

export function loadDemoWorkspace(withGit: boolean) {
  const clean: GitStatusSnapshot = {
    branch: 'main',
    upstream: 'origin/main',
    ahead: 1,
    behind: 0,
    stagedCount: 0,
    changedCount: 2,
    untrackedCount: 1,
    conflictedCount: 0,
    files: [
      {
        path: 'README.md',
        indexStatus: 'clean',
        worktreeStatus: 'modified',
        staged: false,
        conflicted: false,
        untracked: false,
      },
      {
        path: 'notes/ideas.md',
        indexStatus: 'clean',
        worktreeStatus: 'untracked',
        staged: false,
        conflicted: false,
        untracked: true,
      },
    ],
  }
  const workspace = reactive<WorkspaceDescriptor>({
    id: 'marktree-demo',
    name: 'Marktree',
    root: 'E:\\Writing\\Marktree',
    git: withGit
      ? {
          commonDir: 'E:\\Writing\\Marktree\\.git',
          remoteUrl: 'https://github.com/example/marktree-notes',
          status: clean,
          worktrees: [
            {
              name: 'main',
              path: 'E:\\Writing\\Marktree',
              branch: 'main',
              isMain: true,
              isLocked: false,
              isDetached: false,
              status: clean,
            },
            {
              name: 'book',
              path: 'E:\\Writing\\Marktree-book',
              branch: 'book',
              isMain: false,
              isLocked: false,
              isDetached: false,
              status: { ...clean, changedCount: 0, files: [] },
            },
          ],
        }
      : null,
  })
  workspaces.value = [workspace]
  activeWorkspaceId.value = workspace.id
  activeWorktreePath.value = workspace.root
  const session = ensureSession(workspace.root)
  session.branches = withGit
    ? [
        {
          name: 'main',
          isCurrent: true,
          upstream: 'origin/main',
          ahead: 1,
          behind: 0,
          checkedOutPath: workspace.root,
        },
        {
          name: 'book',
          isCurrent: false,
          upstream: null,
          ahead: 0,
          behind: 0,
          checkedOutPath: 'E:\\Writing\\Marktree-book',
        },
      ]
    : []
  session.entries = [
    {
      path: 'notes',
      name: 'notes',
      entryType: 'directory' as const,
      fileKind: null,
      size: 0,
      modifiedMs: Date.now(),
      readOnly: true,
      gitStatus: null,
    },
    {
      path: 'docs',
      name: 'docs',
      entryType: 'directory' as const,
      fileKind: null,
      size: 0,
      modifiedMs: Date.now(),
      readOnly: true,
      gitStatus: null,
    },
    ...Object.keys(demoContents).map((path) => ({
      path,
      name: fileName(path),
      entryType: 'file' as const,
      fileKind: 'markdown' as const,
      size: demoContents[path]?.length ?? 0,
      modifiedMs: Date.now(),
      readOnly: false,
      gitStatus: withGit
        ? (clean.files.find((file) => file.path === path) ?? null)
        : null,
    })),
  ]
  void openDocument('README.md')
}
