import { GFM, parser } from '@lezer/markdown'

export interface MarkdownOutlineEntry {
  text: string
  level: number
  position: number
}

const markdownParser = parser.configure(GFM)

export function markdownOutline(source: string): MarkdownOutlineEntry[] {
  const entries: MarkdownOutlineEntry[] = []
  const cursor = markdownParser.parse(source).cursor()
  do {
    const match = /^(?:ATX|Setext)Heading([1-6])$/.exec(cursor.name)
    if (!match) continue
    const raw = source.slice(cursor.from, cursor.to)
    const firstLine = raw.split(/\r\n|\r|\n/, 1)[0] ?? ''
    const text = firstLine
      .replace(/^#{1,6}[\t ]+/, '')
      .replace(/[\t ]+#+[\t ]*$/, '')
      .trim()
    if (text) {
      entries.push({
        text,
        level: Number(match[1]),
        position: cursor.from,
      })
    }
  } while (cursor.next())
  return entries
}
