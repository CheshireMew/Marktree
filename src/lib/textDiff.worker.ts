/// <reference lib="webworker" />

import { createTextDiffResult, type TextDiffRequest } from './textDiff'

self.addEventListener('message', (event: MessageEvent<{ id: number; request: TextDiffRequest }>) => {
  self.postMessage({ id: event.data.id, result: createTextDiffResult(event.data.request) })
})
