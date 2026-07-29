import { EditorState, Text, type ChangeSet, type Extension } from '@codemirror/state'

export type LineSeparator = '\n' | '\r\n' | '\r'

export function detectLineSeparator(source: string): LineSeparator {
  const match = /\r\n|\r|\n/.exec(source)
  return (match?.[0] as LineSeparator | undefined) ?? '\n'
}

export function sourceDocument(source: string): Text {
  return Text.of(source.split(/\r\n|\r|\n/))
}

export function sourceLineSeparator(source: string): Extension {
  return EditorState.lineSeparator.of(detectLineSeparator(source))
}

export function editorSourceText(state: EditorState): string {
  return state.sliceDoc()
}

export function sourceOffsetFromEditor(source: string, editorOffset: number): number {
  const editorLength = sourceDocument(source).length
  const target = Math.max(0, Math.min(editorOffset, editorLength))
  let sourceOffset = 0
  let normalizedOffset = 0
  while (sourceOffset < source.length && normalizedOffset < target) {
    if (source[sourceOffset] === '\r' && source[sourceOffset + 1] === '\n') {
      sourceOffset += 2
    } else {
      sourceOffset += 1
    }
    normalizedOffset += 1
  }
  return sourceOffset
}

export function editorOffsetFromSource(source: string, sourceOffset: number): number {
  const target = Math.max(0, Math.min(sourceOffset, source.length))
  return sourceDocument(source.slice(0, target)).length
}

export function applyEditorChangesToSource(
  source: string,
  changes: ChangeSet,
  insertedLineSeparator: LineSeparator,
) {
  const output: string[] = []
  let sourceCursor = 0
  changes.iterChanges((fromA, toA, _fromB, _toB, inserted) => {
    const sourceFrom = sourceOffsetFromEditor(source, fromA)
    const sourceTo = sourceOffsetFromEditor(source, toA)
    output.push(source.slice(sourceCursor, sourceFrom))
    output.push(inserted.sliceString(0, inserted.length, insertedLineSeparator))
    sourceCursor = sourceTo
  })
  output.push(source.slice(sourceCursor))
  return output.join('')
}
