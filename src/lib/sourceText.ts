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

function crlfOffsets(source: string) {
  const offsets: number[] = []
  let cursor = source.indexOf('\r\n')
  while (cursor >= 0) {
    offsets.push(cursor)
    cursor = source.indexOf('\r\n', cursor + 2)
  }
  return offsets
}

function countBefore(offsets: readonly number[], target: number) {
  let low = 0
  let high = offsets.length
  while (low < high) {
    const middle = (low + high) >>> 1
    if ((offsets[middle] ?? Number.POSITIVE_INFINITY) < target) low = middle + 1
    else high = middle
  }
  return low
}

/**
 * Keeps the source-faithful text and the editor/source offset projection together.
 * CodeMirror normalizes CRLF to one character, so rebuilding a Text document for
 * every offset lookup turns each keystroke into repeated whole-document scans.
 */
export class SourceTextBuffer {
  private crlf: number[]

  constructor(private value: string) {
    this.crlf = crlfOffsets(value)
  }

  get source() {
    return this.value
  }

  replace(source: string) {
    this.value = source
    this.crlf = crlfOffsets(source)
  }

  sourceOffset(editorOffset: number) {
    const editorLength = this.value.length - this.crlf.length
    const target = Math.max(0, Math.min(editorOffset, editorLength))
    let projected = target
    while (true) {
      const next = target + countBefore(this.crlf, projected)
      if (next === projected) return next
      projected = next
    }
  }

  editorOffset(sourceOffset: number) {
    const target = Math.max(0, Math.min(sourceOffset, this.value.length))
    return target - countBefore(this.crlf, target)
  }

  apply(changes: ChangeSet, insertedLineSeparator: LineSeparator) {
    const output: string[] = []
    const edits: Array<{ from: number; to: number; inserted: string }> = []
    let sourceCursor = 0
    changes.iterChanges((fromA, toA, _fromB, _toB, inserted) => {
      const sourceFrom = this.sourceOffset(fromA)
      const sourceTo = this.sourceOffset(toA)
      const insertedSource = inserted.sliceString(0, inserted.length, insertedLineSeparator)
      output.push(this.value.slice(sourceCursor, sourceFrom))
      output.push(insertedSource)
      edits.push({ from: sourceFrom, to: sourceTo, inserted: insertedSource })
      sourceCursor = sourceTo
    })
    output.push(this.value.slice(sourceCursor))
    const nextValue = output.join('')
    const retained: number[] = []
    for (const offset of this.crlf) {
      if (edits.some((edit) => offset < edit.to && offset + 2 > edit.from)) continue
      let delta = 0
      for (const edit of edits) {
        if (edit.to <= offset) delta += edit.inserted.length - (edit.to - edit.from)
        else break
      }
      retained.push(offset + delta)
    }
    let delta = 0
    for (const edit of edits) {
      const from = edit.from + delta
      const to = from + edit.inserted.length
      const scanFrom = Math.max(0, from - 1)
      const scanTo = Math.min(nextValue.length, to + 1)
      let cursor = nextValue.indexOf('\r\n', scanFrom)
      while (cursor >= 0 && cursor < scanTo) {
        retained.push(cursor)
        cursor = nextValue.indexOf('\r\n', cursor + 2)
      }
      delta += edit.inserted.length - (edit.to - edit.from)
    }
    retained.sort((left, right) => left - right)
    this.crlf = retained.filter((offset, index) => offset !== retained[index - 1])
    this.value = nextValue
    return this.value
  }
}

export function sourceOffsetFromEditor(source: string, editorOffset: number): number {
  return new SourceTextBuffer(source).sourceOffset(editorOffset)
}

export function editorOffsetFromSource(source: string, sourceOffset: number): number {
  return new SourceTextBuffer(source).editorOffset(sourceOffset)
}

export function applyEditorChangesToSource(
  source: string,
  changes: ChangeSet,
  insertedLineSeparator: LineSeparator,
) {
  return new SourceTextBuffer(source).apply(changes, insertedLineSeparator)
}
