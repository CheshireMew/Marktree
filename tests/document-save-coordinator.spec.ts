import { describe, expect, it, vi } from 'vitest'

import { saveEditorTabUntilStable } from '../src/lib/documentSaveCoordinator'
import type { EditorTab } from '../src/types'

function editorTab(): EditorTab {
  return {
    root: 'C:\\repo',
    path: 'notes/shared.md',
    title: 'shared.md',
    content: 'first edit',
    diskContent: 'original',
    modifiedMs: 1,
    sha256: 'sha-original',
    readOnly: false,
    encoding: 'utf8',
    lineEnding: 'lf',
    revision: 1,
    savedRevision: 0,
    dirty: true,
    saving: false,
  }
}

describe('serialized document saving', () => {
  it('persists an edit made while an earlier save is still in flight', async () => {
    const tab = editorTab()
    let releaseFirst!: () => void
    const firstSave = new Promise<void>((resolve) => {
      releaseFirst = resolve
    })
    const persisted: string[] = []
    const save = vi.fn(
      async (
        _tab: EditorTab,
        content: string,
        _expectedSha256: string | undefined,
        _expectedMissing: boolean,
      ) => {
      if (!persisted.length) await firstSave
      persisted.push(content)
      return {
        path: tab.path,
        modifiedMs: persisted.length + 1,
        sha256: `sha-${persisted.length}`,
        encoding: 'utf8' as const,
        lineEnding: 'lf' as const,
      }
      },
    )

    const saving = saveEditorTabUntilStable(tab, save)
    await Promise.resolve()
    tab.content = 'second edit'
    tab.revision = 2
    tab.dirty = true
    releaseFirst()
    await saving

    expect(persisted).toEqual(['first edit', 'second edit'])
    expect(save.mock.calls[1]?.[2]).toBe('sha-1')
    expect(save.mock.calls[1]?.[3]).toBe(false)
    expect(tab.diskContent).toBe('second edit')
    expect(tab.savedRevision).toBe(2)
    expect(tab.dirty).toBe(false)
  })

  it('keeps failed content dirty and exposes the producer error', async () => {
    const tab = editorTab()
    const failure = { code: 'externalChange', message: 'changed on disk' }
    const onFailure = vi.fn()

    await expect(
      saveEditorTabUntilStable(
        tab,
        async () => {
          throw failure
        },
        undefined,
        onFailure,
      ),
    ).rejects.toBe(failure)

    expect(onFailure).toHaveBeenCalledWith(failure)
    expect(tab.content).toBe('first edit')
    expect(tab.dirty).toBe(true)
    expect(tab.sha256).toBe('sha-original')
  })

  it('keeps a confirmed deletion distinct from an unknown expected hash', async () => {
    const tab = editorTab()
    const save = vi.fn(async () => ({
      path: tab.path,
      modifiedMs: 2,
      sha256: 'sha-recreated',
      encoding: 'utf8' as const,
      lineEnding: 'lf' as const,
    }))

    await saveEditorTabUntilStable(tab, save, null)

    expect(save).toHaveBeenCalledWith(tab, 'first edit', undefined, true)
  })
})
