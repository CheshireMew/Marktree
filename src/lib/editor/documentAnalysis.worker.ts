import { markdownOutline } from './outline'

self.onmessage = (event: MessageEvent<{ id: number; source: string }>) => {
  self.postMessage({ id: event.data.id, headings: markdownOutline(event.data.source) })
}
