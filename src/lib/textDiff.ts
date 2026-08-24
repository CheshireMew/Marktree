import { diffLines } from 'diff'

import type { WorkspaceDiffResult } from '@/types'

export interface TextDiffRequest {
  mode: Extract<WorkspaceDiffResult['mode'], 'worktreeToWorktree' | 'unsavedToDisk'>
  oldLabel: string
  newLabel: string
  path: string
  header: string
  oldText: string
  newText: string
}

const MAX_RENDERED_DIFF_LINES = 20_000

export function createTextDiffResult(request: TextDiffRequest): WorkspaceDiffResult {
  const lines = []
  let oldLine = 1
  let newLine = 1
  let insertions = 0
  let deletions = 0
  let omittedLines = 0

  for (const part of diffLines(request.oldText, request.newText)) {
    for (const content of part.value.match(/[^\n]*\n|[^\n]+$/g) ?? []) {
      if (!content) continue
      const visible = lines.length < MAX_RENDERED_DIFF_LINES
      if (part.added) {
        if (visible) lines.push({
          kind: 'addition' as const,
          oldLine: null,
          newLine: newLine++,
          content,
        })
        insertions += 1
      } else if (part.removed) {
        if (visible) lines.push({
          kind: 'deletion' as const,
          oldLine: oldLine++,
          newLine: null,
          content,
        })
        deletions += 1
      } else {
        if (visible) lines.push({
          kind: 'context' as const,
          oldLine: oldLine++,
          newLine: newLine++,
          content,
        })
      }
      if (!visible) omittedLines += 1
    }
  }

  return {
    mode: request.mode,
    oldLabel: request.oldLabel,
    newLabel: request.newLabel,
    insertions,
    deletions,
    truncated: omittedLines > 0,
    omittedLines,
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

type PendingDiff = {
  resolve: (value: WorkspaceDiffResult) => void
  reject: (reason: unknown) => void
}

let worker: Worker | undefined
let nextRequestId = 0
const pending = new Map<number, PendingDiff>()

function textDiffWorker() {
  if (worker) return worker
  worker = new Worker(new URL('./textDiff.worker.ts', import.meta.url), { type: 'module' })
  worker.addEventListener('message', (event: MessageEvent<{ id: number; result: WorkspaceDiffResult }>) => {
    const request = pending.get(event.data.id)
    if (!request) return
    pending.delete(event.data.id)
    request.resolve(event.data.result)
  })
  worker.addEventListener('error', (event) => {
    for (const request of pending.values()) request.reject(event.error ?? new Error(event.message))
    pending.clear()
    worker?.terminate()
    worker = undefined
  })
  return worker
}

export function createTextDiffResultAsync(request: TextDiffRequest) {
  if (typeof Worker === 'undefined') return Promise.resolve(createTextDiffResult(request))
  const id = ++nextRequestId
  return new Promise<WorkspaceDiffResult>((resolve, reject) => {
    pending.set(id, { resolve, reject })
    textDiffWorker().postMessage({ id, request })
  })
}
