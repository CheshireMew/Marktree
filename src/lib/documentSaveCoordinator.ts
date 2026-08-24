import type { EditorTab, SaveDocumentResult } from '@/types'
import { editorTabIsDirty, retainedDiskContent } from '@/lib/documentMemory'

export type SaveDocumentSnapshot = (
  tab: EditorTab,
  content: string,
  expectedSha256: string | undefined,
  expectedMissing: boolean,
) => Promise<SaveDocumentResult>

export async function saveEditorTabUntilStable(
  tab: EditorTab,
  saveSnapshot: SaveDocumentSnapshot,
  expectedShaOverride?: string | null,
  onFailure?: (reason: unknown) => void | Promise<void>,
) {
  tab.saving = true
  let nextExpectedSha = expectedShaOverride
  while (tab.dirty) {
    const snapshotRevision = tab.revision
    const snapshotContent = tab.content
    const expectedSha =
      nextExpectedSha === null ? undefined : (nextExpectedSha ?? tab.sha256)
    const expectedMissing = nextExpectedSha === null
    nextExpectedSha = undefined
    try {
      const saved = await saveSnapshot(
        tab,
        snapshotContent,
        expectedSha,
        expectedMissing,
      )
      tab.sha256 = saved.sha256
      tab.modifiedMs = saved.modifiedMs
      tab.encoding = saved.encoding
      tab.lineEnding = saved.lineEnding
      tab.diskContent = retainedDiskContent(snapshotContent)
      tab.savedRevision = snapshotRevision
      tab.dirty = tab.revision !== snapshotRevision || editorTabIsDirty(tab)
      tab.saveError = undefined
    } catch (reason) {
      await onFailure?.(reason)
      throw reason
    }
  }
}
