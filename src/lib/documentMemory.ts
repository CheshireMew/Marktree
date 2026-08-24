import type { EditorTab } from '@/types'

export const LARGE_DOCUMENT_CHARACTERS = 2 * 1024 * 1024
export const MAX_EDITABLE_DOCUMENT_CHARACTERS = 32 * 1024 * 1024
export const MAX_OPEN_DOCUMENT_CHARACTERS = 64 * 1024 * 1024
export const MAX_OPEN_DOCUMENTS = 64

export function retainedDiskContent(content: string) {
  return content.length < LARGE_DOCUMENT_CHARACTERS ? content : undefined
}

export function editorTabIsDirty(tab: EditorTab) {
  return tab.diskContent === undefined
    ? tab.revision !== tab.savedRevision
    : tab.content !== tab.diskContent
}

export function canOpenDocument(tabs: readonly EditorTab[], content: string) {
  return tabs.length < MAX_OPEN_DOCUMENTS &&
    tabs.reduce((total, tab) => total + tab.content.length, content.length) <=
      MAX_OPEN_DOCUMENT_CHARACTERS
}

export function editableDocumentCharacterLimit(
  tabs: readonly EditorTab[],
  activeTab?: EditorTab,
) {
  const retainedByOtherTabs = tabs.reduce(
    (total, tab) => total + (tab === activeTab ? 0 : tab.content.length),
    0,
  )
  const available = Math.max(0, MAX_OPEN_DOCUMENT_CHARACTERS - retainedByOtherTabs)
  return Math.max(
    activeTab?.content.length ?? 0,
    Math.min(MAX_EDITABLE_DOCUMENT_CHARACTERS, available),
  )
}
