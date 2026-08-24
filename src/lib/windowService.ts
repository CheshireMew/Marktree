import type { UnlistenFn } from '@tauri-apps/api/event'
import { WebviewWindow } from '@tauri-apps/api/webviewWindow'
import {
  getCurrentWindow,
  type CloseRequestedEvent,
  type PhysicalSize,
} from '@tauri-apps/api/window'

export const windowService = {
  isMaximized: () => getCurrentWindow().isMaximized(),
  minimize: () => getCurrentWindow().minimize(),
  toggleMaximize: () => getCurrentWindow().toggleMaximize(),
  hide: () => getCurrentWindow().hide(),
  requestClose: () => getCurrentWindow().close(),
  destroyAfterFlush: () => getCurrentWindow().destroy(),
  onResized: (
    handler: (event: { payload: PhysicalSize }) => void,
  ): Promise<UnlistenFn> => getCurrentWindow().onResized(handler),
  onCloseRequested: (
    handler: (event: CloseRequestedEvent) => void | Promise<void>,
  ): Promise<UnlistenFn> => getCurrentWindow().onCloseRequested(handler),
  async openWorkspaceWindow(options: {
    root: string
    worktree: string
    title: string
  }): Promise<void> {
    const label = `workspace-${crypto.randomUUID()}`
    const webview = new WebviewWindow(label, {
      url: `/?root=${encodeURIComponent(options.root)}&worktree=${encodeURIComponent(options.worktree)}`,
      title: options.title,
      width: 1280,
      height: 820,
      minWidth: 900,
      minHeight: 600,
      decorations: false,
    })
    await new Promise<void>((resolve, reject) => {
      void webview.once('tauri://created', () => resolve())
      void webview.once<unknown>('tauri://error', (event) => reject(event.payload))
    })
  },
}
