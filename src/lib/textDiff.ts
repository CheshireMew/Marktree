import { diffLines } from 'diff'

import type { WorkspaceDiffResult } from '@/types'

interface TextDiffRequest {
  mode: Extract<WorkspaceDiffResult['mode'], 'worktreeToWorktree' | 'unsavedToDisk'>
  oldLabel: string
  newLabel: string
  path: string
  header: string
  oldText: string
  newText: string
}

export function createTextDiffResult(request: TextDiffRequest): WorkspaceDiffResult {
  const lines = []
  let oldLine = 1
  let newLine = 1
  let insertions = 0
  let deletions = 0

  for (const part of diffLines(request.oldText, request.newText)) {
    for (const content of part.value.match(/[^\n]*\n|[^\n]+$/g) ?? []) {
      if (!content) continue
      if (part.added) {
        lines.push({
          kind: 'addition' as const,
          oldLine: null,
          newLine: newLine++,
          content,
        })
        insertions += 1
      } else if (part.removed) {
        lines.push({
          kind: 'deletion' as const,
          oldLine: oldLine++,
          newLine: null,
          content,
        })
        deletions += 1
      } else {
        lines.push({
          kind: 'context' as const,
          oldLine: oldLine++,
          newLine: newLine++,
          content,
        })
      }
    }
  }

  return {
    mode: request.mode,
    oldLabel: request.oldLabel,
    newLabel: request.newLabel,
    insertions,
    deletions,
    files: [
      {
        path: request.path,
        oldPath: null,
        status: 'modified',
        binary: false,
        hunks: [
          {
            header: request.header,
            oldStart: 1,
            oldLines: oldLine - 1,
            newStart: 1,
            newLines: newLine - 1,
            lines,
          },
        ],
      },
    ],
  }
}
