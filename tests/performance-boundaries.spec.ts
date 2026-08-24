import { describe, expect, it } from 'vitest'

import {
  canOpenDocument,
  editableDocumentCharacterLimit,
  editorTabIsDirty,
  MAX_EDITABLE_DOCUMENT_CHARACTERS,
  MAX_OPEN_DOCUMENT_CHARACTERS,
  MAX_OPEN_DOCUMENTS,
  retainedDiskContent,
} from '../src/lib/documentMemory'
import { createTextDiffResult } from '../src/lib/textDiff'
import type { EditorTab } from '../src/types'

function editorTab(content: string): EditorTab {
  return {
    root: 'C:\\workspace',
    path: `${content.length}.md`,
    title: 'document.md',
    content,
    diskContent: retainedDiskContent(content),
    modifiedMs: 0,
    sha256: '',
    readOnly: false,
    encoding: 'utf8',
    lineEnding: 'none',
    revision: 0,
    savedRevision: 0,
    dirty: false,
    saving: false,
  }
}

describe('performance boundaries', () => {
  it('caps rendered text diff lines while retaining complete totals', () => {
    const oldText = `${'old\n'.repeat(20_050)}`
    const result = createTextDiffResult({
      mode: 'unsavedToDisk',
      oldLabel: 'disk',
      newLabel: 'editor',
      path: 'large.md',
      header: 'large',
      oldText,
      newText: '',
    })

    expect(result.deletions).toBe(20_050)
    expect(result.files[0]?.hunks[0]?.lines).toHaveLength(20_000)
    expect(result.truncated).toBe(true)
    expect(result.omittedLines).toBe(50)
  })

  it('does not retain a second full string for a large editor tab', () => {
    const content = 'x'.repeat(2 * 1024 * 1024)
    const tab = {
      root: 'C:\\notes',
      path: 'large.md',
      title: 'large.md',
      content,
      diskContent: retainedDiskContent(content),
      modifiedMs: 1,
      sha256: 'same',
      readOnly: false,
      encoding: 'utf8',
      lineEnding: 'lf',
      revision: 2,
      savedRevision: 2,
      dirty: false,
      saving: false,
    } satisfies EditorTab

    expect(tab.diskContent).toBeUndefined()
    expect(editorTabIsDirty(tab)).toBe(false)
    tab.revision += 1
    expect(editorTabIsDirty(tab)).toBe(true)
  })

  it('bounds both the count and aggregate content of open documents', () => {
    expect(canOpenDocument(Array.from({ length: MAX_OPEN_DOCUMENTS }, () => editorTab('x')), 'x'))
      .toBe(false)
    expect(canOpenDocument([editorTab('x'.repeat(MAX_OPEN_DOCUMENT_CHARACTERS))], 'x'))
      .toBe(false)
    expect(canOpenDocument([editorTab('small')], 'next')).toBe(true)
  })

  it('limits active growth by both the document and session budgets', () => {
    const active = editorTab('active')
    expect(editableDocumentCharacterLimit([active], active))
      .toBe(MAX_EDITABLE_DOCUMENT_CHARACTERS)
    const retained = editorTab('x'.repeat(MAX_OPEN_DOCUMENT_CHARACTERS - 10))
    expect(editableDocumentCharacterLimit([retained, active], active)).toBe(10)
  })
})
