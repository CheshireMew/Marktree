import { reactive } from 'vue'

import type { GitStatusSnapshot, RepositoryDescriptor } from '@/types'

import {
  activeRepositoryId,
  activeWorktreePath,
  ensureSession,
  fileName,
  repositories,
} from './state'
import { openDocument } from './documents'
import { demoContents } from './demoData'

export function loadDemoWorkspace() {
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
  const repository = reactive<RepositoryDescriptor>({
    id: 'marktree-demo',
    name: 'Marktree',
    root: 'E:\\Writing\\Marktree',
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
  })
  repositories.value = [repository]
  activeRepositoryId.value = repository.id
  activeWorktreePath.value = repository.root
  const session = ensureSession(repository.root)
  session.branches = [
    {
      name: 'main',
      isCurrent: true,
      upstream: 'origin/main',
      ahead: 1,
      behind: 0,
      checkedOutPath: repository.root,
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
  session.documents = Object.keys(demoContents).map((path) => ({
    path,
    name: fileName(path),
    extension: 'md',
    size: demoContents[path]?.length ?? 0,
    modifiedMs: Date.now(),
    readOnly: false,
    kind: 'markdown',
    gitStatus: clean.files.find((file) => file.path === path) ?? null,
  }))
  void openDocument('README.md')
}
