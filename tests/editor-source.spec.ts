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
  SourceTextBuffer,
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

  it('updates CRLF projections incrementally across multiple edits', () => {
    const source = 'first\r\nsecond\r\nthird\nfourth'
    const state = EditorState.create({
      doc: sourceDocument(source),
      extensions: [sourceLineSeparator(source)],
    })
    const transaction = state.update({
      changes: [
        { from: 0, to: 5, insert: '1' },
        {
          from: state.doc.toString().indexOf('third'),
          to: state.doc.toString().indexOf('third') + 5,
          insert: sourceDocument('3\nthree'),
        },
      ],
    })
    const buffer = new SourceTextBuffer(source)

    expect(buffer.apply(transaction.changes, '\r\n')).toBe(
      '1\r\nsecond\r\n3\r\nthree\nfourth',
    )
    expect(buffer.sourceOffset(2)).toBe(3)
    expect(buffer.editorOffset(3)).toBe(2)
  })

  it('keeps a multi-megabyte single edit responsive', () => {
    const source = `${'line\r\n'.repeat(400_000)}tail`
    const state = EditorState.create({ doc: sourceDocument(source) })
    const buffer = new SourceTextBuffer(source)
    const started = performance.now()
    const transaction = state.update({ changes: { from: 0, to: 4, insert: 'head' } })

    expect(buffer.apply(transaction.changes, '\r\n').endsWith('tail')).toBe(true)
    expect(performance.now() - started).toBeLessThan(750)
  })
})
