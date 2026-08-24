import { watch, type Ref } from 'vue'

import type { WorkspaceApi } from '@/composables/useWorkspace'
import { isTauri, nativeApi } from '@/lib/api'
import { readableError } from '@/lib/errors'
import { windowService } from '@/lib/windowService'

type Workspace = Pick<
  WorkspaceApi,
  | 'activeTab'
  | 'addWorkspace'
  | 'createDocument'
  | 'entries'
  | 'openDocument'
  | 'reportError'
  | 'updateActiveContent'
>

const DOCUMENT_PATH = 'native-smoke.md'
const FIRST_CONTENT = '# Marktree native smoke\n\nSaved through the editor close lifecycle.\n'
const SECOND_CONTENT = '# Marktree native smoke\n\nReopened, rendered, and saved again.\n'

export function useNativeSmoke(workspace: Workspace, ready: Ref<boolean>) {
  const root = import.meta.env.VITE_MARKTREE_SMOKE_ROOT?.trim()
  const phase = import.meta.env.VITE_MARKTREE_SMOKE_PHASE?.trim()
  if (!import.meta.env.DEV || !isTauri() || !root || !['write', 'verify'].includes(phase)) {
    return
  }

  let started = false
  watch(
    ready,
    (isReady) => {
      if (!isReady || started) return
      started = true
      void runNativeSmoke(workspace, root, phase as 'write' | 'verify').catch(
        async (reason) => {
          const message = `Native smoke failed: ${readableError(reason)}`
          workspace.reportError(message)
          console.error(message)
          await windowService.requestClose()
        },
      )
    },
    { immediate: true },
  )
}

async function runNativeSmoke(
  workspace: Workspace,
  root: string,
  phase: 'write' | 'verify',
) {
  await windowService.hide()
  const descriptor = await nativeApi.openWorkspace({ path: root })
  await workspace.addWorkspace(descriptor)

  if (phase === 'write') {
    if (!workspace.entries.value.some((entry) => entry.path === DOCUMENT_PATH)) {
      const created = await workspace.createDocument(DOCUMENT_PATH)
      if (!created) throw new Error('The smoke document could not be created.')
    } else {
      await workspace.openDocument(DOCUMENT_PATH)
    }
    assertActiveDocument(workspace, '')
    workspace.updateActiveContent(FIRST_CONTENT)
    await waitForEditorText('Saved through the editor close lifecycle.')
  } else {
    await workspace.openDocument(DOCUMENT_PATH)
    assertActiveDocument(workspace, FIRST_CONTENT)
    await waitForEditorText('Saved through the editor close lifecycle.')
    workspace.updateActiveContent(SECOND_CONTENT)
    await waitForEditorText('Reopened, rendered, and saved again.')
  }

  await windowService.requestClose()
}

function assertActiveDocument(workspace: Workspace, expectedContent: string) {
  const tab = workspace.activeTab.value
  if (!tab || tab.path !== DOCUMENT_PATH) {
    throw new Error('The smoke document was not opened in the editor.')
  }
  if (tab.content !== expectedContent) {
    throw new Error('The editor did not read the exact content saved by the previous run.')
  }
}

async function waitForEditorText(expected: string) {
  const deadline = Date.now() + 15_000
  while (Date.now() < deadline) {
    const content = document.querySelector('.cm-content')?.textContent ?? ''
    if (content.includes(expected)) return
    await new Promise((resolve) => window.setTimeout(resolve, 100))
  }
  throw new Error(`The rendered editor never displayed: ${expected}`)
}
