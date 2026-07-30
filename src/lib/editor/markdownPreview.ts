import { syntaxTree } from '@codemirror/language'
import { EditorState, StateField, type Extension, type Range } from '@codemirror/state'
import {
  Decoration,
  EditorView,
  ViewPlugin,
  type DecorationSet,
  type ViewUpdate,
} from '@codemirror/view'

import {
  FootnoteDefinitionWidget,
  FootnoteReferenceWidget,
  FrontmatterWidget,
  ImageWidget,
  MathWidget,
  MermaidWidget,
  TableWidget,
  TaskWidget,
  type ResolvedImageSource,
} from './previewWidgets'
import type { WorkspaceImageLoader } from '@/types'

export interface MarkdownImageContext {
  root?: string
  path?: string
  loadWorkspaceImage: WorkspaceImageLoader
}

function liveMarkdownDecorations(
  view: EditorView,
  imageContext: MarkdownImageContext,
): DecorationSet {
  const cursor = view.state.selection.main.head
  const ranges: Range<Decoration>[] = []
  const hiddenMarks = new Set([
    'HeaderMark',
    'EmphasisMark',
    'CodeMark',
    'LinkMark',
    'QuoteMark',
    'ListMark',
  ])
  const classes: Record<string, string> = {
    ATXHeading1: 'cm-heading cm-heading-1',
    ATXHeading2: 'cm-heading cm-heading-2',
    ATXHeading3: 'cm-heading cm-heading-3',
    ATXHeading4: 'cm-heading cm-heading-4',
    ATXHeading5: 'cm-heading cm-heading-5',
    ATXHeading6: 'cm-heading cm-heading-6',
    StrongEmphasis: 'cm-strong',
    Emphasis: 'cm-emphasis',
    InlineCode: 'cm-inline-code',
    Blockquote: 'cm-blockquote',
    Link: 'cm-link',
  }

  syntaxTree(view.state).iterate({
    enter(node) {
      const className = classes[node.name]
      if (className) ranges.push(Decoration.mark({ class: className }).range(node.from, node.to))
      if (hiddenMarks.has(node.name)) {
        const structure = syntaxTree(view.state).resolve(node.from, 1).parent
        const editing =
          cursor >= (structure?.from ?? node.from) && cursor <= (structure?.to ?? node.to)
        if (!editing) {
          ranges.push(
            Decoration.mark({ class: 'cm-markdown-mark-hidden' }).range(node.from, node.to),
          )
        }
      }
    },
  })

  const text = view.state.doc.toString()
  const footnotes = footnoteDefinitions(text)
  const footnoteReferencePattern = /\[\^([^\]\r\n]+)\]/g
  for (const match of text.matchAll(footnoteReferencePattern)) {
    const from = match.index
    const to = from + match[0].length
    const line = view.state.doc.lineAt(from)
    const before = text.slice(line.from, from)
    if (!before.trim() && text[to] === ':') continue
    if (cursor < from || cursor > to) {
      const label = match[1] ?? ''
      ranges.push(
        Decoration.replace({
          widget: new FootnoteReferenceWidget(label, footnotes.get(label) ?? label),
        }).range(from, to),
      )
    }
  }
  const inlineMathPattern = /(?<![\\$])\$([^$\n]+?)(?<![\\$])\$(?!\$)/g
  for (const match of text.matchAll(inlineMathPattern)) {
    const from = match.index
    const to = from + match[0].length
    if (cursor < from || cursor > to) {
      ranges.push(
        Decoration.replace({
          widget: new MathWidget(match[1] ?? '', false),
        }).range(from, to),
      )
    }
  }

  const imagePattern = /!\[([^\]]*)\]\(([^)\s]+)(?:\s+["'][^"']*["'])?\)/g
  for (const match of text.matchAll(imagePattern)) {
    const from = match.index
    const to = from + match[0].length
    if (cursor < from || cursor > to) {
      const source = resolveImageSource(match[2] ?? '', imageContext)
      ranges.push(
        Decoration.replace({
          widget: new ImageWidget(
            source,
            match[1] ?? '',
            imageContext.loadWorkspaceImage,
          ),
        }).range(from, to),
      )
    }
  }

  const taskPattern = /^(\s*[-*+]\s+)(\[([ xX])\])/gm
  for (const match of text.matchAll(taskPattern)) {
    const from = match.index + (match[1]?.length ?? 0)
    const marker = match[2] ?? '[ ]'
    const to = from + marker.length
    const line = view.state.doc.lineAt(from)
    if (cursor < line.from || cursor > line.to) {
      ranges.push(
        Decoration.replace({
          widget: new TaskWidget((match[3] ?? '').toLowerCase() === 'x', from, to),
        }).range(from, to),
      )
    }
  }

  return Decoration.set(ranges, true)
}

function blockPreviewDecorations(state: EditorState): DecorationSet {
  const cursor = state.selection.main.head
  const text = state.doc.toString()
  const ranges: Range<Decoration>[] = []
  const footnotePattern = /^\[\^([^\]\r\n]+)\]:[ \t]*(.*(?:\r?\n(?: {2,}|\t).*)*)/gm
  for (const match of text.matchAll(footnotePattern)) {
    const from = match.index
    const to = from + match[0].length
    if (cursor < from || cursor > to) {
      ranges.push(
        Decoration.replace({
          widget: new FootnoteDefinitionWidget(
            match[1] ?? '',
            (match[2] ?? '').replace(/\r?\n(?: {2,}|\t)/g, '\n'),
          ),
          block: true,
        }).range(from, to),
      )
    }
  }
  const frontmatter = /^(---\s*\r?\n[\s\S]*?\r?\n---)\s*(?:\r?\n|$)/.exec(text)
  if (frontmatter) {
    const from = 0
    const to = frontmatter[1]?.length ?? 0
    if (cursor < from || cursor > to) {
      ranges.push(
        Decoration.replace({
          widget: new FrontmatterWidget(frontmatter[1] ?? ''),
          block: true,
        }).range(from, to),
      )
    }
  }
  const mermaidPattern = /^```mermaid[^\n]*\r?\n([\s\S]*?)^```\s*$/gm
  for (const match of text.matchAll(mermaidPattern)) {
    const from = match.index
    const to = from + match[0].length
    if (cursor < from || cursor > to) {
      ranges.push(
        Decoration.replace({
          widget: new MermaidWidget(match[1] ?? ''),
          block: true,
        }).range(from, to),
      )
    }
  }

  const blockMathPattern = /^\$\$\s*\r?\n([\s\S]*?)^\$\$\s*$/gm
  for (const match of text.matchAll(blockMathPattern)) {
    const from = match.index
    const to = from + match[0].length
    if (cursor < from || cursor > to) {
      ranges.push(
        Decoration.replace({
          widget: new MathWidget((match[1] ?? '').trim(), true),
          block: true,
        }).range(from, to),
      )
    }
  }

  const tablePattern =
    /^(?:\|?[^\n|]+\|[^\n]*\r?\n)\|?\s*:?-{3,}:?\s*(?:\|\s*:?-{3,}:?\s*)+\|?\r?\n(?:\|?[^\n]+\|[^\n]*(?:\r?\n|$))+/gm
  for (const match of text.matchAll(tablePattern)) {
    const from = match.index
    const to = from + match[0].trimEnd().length
    if (cursor < from || cursor > to) {
      ranges.push(
        Decoration.replace({
          widget: new TableWidget(match[0]),
          block: true,
        }).range(from, to),
      )
    }
  }
  return Decoration.set(ranges, true)
}

function footnoteDefinitions(text: string) {
  const definitions = new Map<string, string>()
  const pattern = /^\[\^([^\]\r\n]+)\]:[ \t]*(.*(?:\r?\n(?: {2,}|\t).*)*)/gm
  for (const match of text.matchAll(pattern)) {
    definitions.set(
      match[1] ?? '',
      (match[2] ?? '').replace(/\r?\n(?: {2,}|\t)/g, '\n'),
    )
  }
  return definitions
}

const blockPreviews = StateField.define<DecorationSet>({
  create: blockPreviewDecorations,
  update(_value, transaction) {
    return blockPreviewDecorations(transaction.state)
  },
  provide: (field) => EditorView.decorations.from(field),
})

function resolveImageSource(
  source: string,
  imageContext: MarkdownImageContext,
): ResolvedImageSource {
  const root = imageContext.root
  if (/^(?:https?:|data:|blob:)/i.test(source) || !root) {
    return { kind: 'external', key: source, url: source }
  }
  let decoded: string
  try {
    decoded = decodeURIComponent(source)
  } catch {
    return { kind: 'external', key: source, url: '' }
  }
  const fromWorkspaceRoot = decoded.startsWith('/') || decoded.startsWith('\\')
  const directory = fromWorkspaceRoot
    ? []
    : (imageContext.path ?? '').replaceAll('\\', '/').split('/').slice(0, -1)
  const parts = [...directory, ...decoded.replaceAll('\\', '/').split('/')]
  const normalized: string[] = []
  for (const part of parts) {
    if (!part || part === '.') continue
    if (part === '..') {
      if (!normalized.length) {
        return { kind: 'external', key: source, url: '' }
      }
      normalized.pop()
    } else {
      normalized.push(part)
    }
  }
  const path = normalized.join('/')
  return {
    kind: 'workspace',
    key: `${root}\n${path}`,
    root,
    path,
  }
}

export function markdownPreviewExtensions(
  imageContext: MarkdownImageContext,
): Extension[] {
  const liveMarkdown = ViewPlugin.fromClass(
    class {
      decorations: DecorationSet

      constructor(view: EditorView) {
        this.decorations = liveMarkdownDecorations(view, imageContext)
      }

      update(update: ViewUpdate) {
        if (update.docChanged || update.selectionSet || update.viewportChanged) {
          this.decorations = liveMarkdownDecorations(update.view, imageContext)
        }
      }
    },
    {
      decorations: (plugin) => plugin.decorations,
    },
  )
  return [liveMarkdown, blockPreviews]
}
