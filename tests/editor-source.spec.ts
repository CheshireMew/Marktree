import { markdown } from '@codemirror/lang-markdown'
import { EditorState } from '@codemirror/state'
import { GFM } from '@lezer/markdown'
import { describe, expect, it } from 'vitest'
import {
  applyEditorChangesToSource,
  detectLineSeparator,
  editorSourceText,
  sourceDocument,
  sourceLineSeparator,
} from '../src/lib/sourceText'

const corpus = [
  '---',
  'title: 中文',
  '---',
  '',
  '# Heading',
  '',
  '| A | B |',
  '| --- | --- |',
  '| 1 | 2 |',
  '',
  '- [x] Task',
  '',
  'Footnote[^1] and $E=mc^2$.',
  '',
  '[^1]: kept',
  '',
  '```mermaid',
  'graph LR',
  'A --> B',
  '```',
  '',
  ':::unknown value',
  'keep exactly',
  ':::',
  '',
].join('\r\n')

describe('source-faithful editor state', () => {
  it('opens the Markdown corpus without changing a byte', () => {
    const state = EditorState.create({
      doc: sourceDocument(corpus),
      extensions: [
        markdown({ extensions: [GFM] }),
        sourceLineSeparator(corpus),
      ],
    })

    expect(editorSourceText(state)).toBe(corpus)
  })

  it('changes only the explicitly edited range', () => {
    const state = EditorState.create({
      doc: sourceDocument(corpus),
      extensions: [
        markdown({ extensions: [GFM] }),
        sourceLineSeparator(corpus),
      ],
    })
    const from = state.doc.toString().indexOf('Heading')
    const to = from + 'Heading'.length
    const next = editorSourceText(
      state.update({
        changes: { from, to, insert: 'Edited heading' },
      }).state,
    )
    const expected = corpus.replace('Heading', 'Edited heading')

    expect(next).toBe(expected)
    expect(next).toContain(':::unknown value\r\nkeep exactly\r\n:::')
  })

  it('preserves mixed line endings outside the edited range', () => {
    const mixed = '# Title\r\nLF line\nCR line\rLast'
    const state = EditorState.create({
      doc: sourceDocument(mixed),
      extensions: [sourceLineSeparator(mixed)],
    })
    const from = state.doc.toString().indexOf('Title')
    const transaction = state.update({
      changes: { from, to: from + 'Title'.length, insert: 'Edited' },
    })

    expect(
      applyEditorChangesToSource(mixed, transaction.changes, detectLineSeparator(mixed)),
    ).toBe('# Edited\r\nLF line\nCR line\rLast')
  })
})
