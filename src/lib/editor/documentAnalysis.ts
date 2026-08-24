import { markdownOutline, type MarkdownOutlineEntry } from './outline'

interface PendingAnalysis {
  resolve: (headings: MarkdownOutlineEntry[]) => void
  reject: (reason: unknown) => void
}

let worker: Worker | undefined
let sequence = 0
const pending = new Map<number, PendingAnalysis>()

function analysisWorker() {
  if (worker || typeof Worker === 'undefined') return worker
  worker = new Worker(new URL('./documentAnalysis.worker.ts', import.meta.url), { type: 'module' })
  worker.addEventListener('message', (event: MessageEvent<{ id: number; headings: MarkdownOutlineEntry[] }>) => {
    const request = pending.get(event.data.id)
    if (!request) return
    pending.delete(event.data.id)
    request.resolve(event.data.headings)
  })
  worker.addEventListener('error', (event) => {
    for (const request of pending.values()) request.reject(event.error ?? event.message)
    pending.clear()
    worker?.terminate()
    worker = undefined
  })
  return worker
}

export async function analyzeMarkdownOutline(source: string) {
  const target = analysisWorker()
  if (!target) return markdownOutline(source)
  const id = ++sequence
  return new Promise<MarkdownOutlineEntry[]>((resolve, reject) => {
    pending.set(id, { resolve, reject })
    target.postMessage({ id, source })
  })
}
