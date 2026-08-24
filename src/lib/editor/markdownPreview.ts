import { syntaxTree } from '@codemirror/language'
import { StateEffect, StateField, type Extension, type Range } from '@codemirror/state'
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
  alwaysPreview: boolean,
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

  const tree = syntaxTree(view.state)
  for (const visible of previewRanges(view)) {
    tree.iterate({
      from: visible.from,
      to: visible.to,
      enter(node) {
        const className = classes[node.name]
        if (className) ranges.push(Decoration.mark({ class: className }).range(node.from, node.to))
        if (hiddenMarks.has(node.name)) {
          const structure = tree.resolve(node.from, 1).parent
          const editing =
            !alwaysPreview &&
            cursor >= (structure?.from ?? node.from) &&
            cursor <= (structure?.to ?? node.to)
          if (!editing) {
            ranges.push(
              Decoration.mark({ class: 'cm-markdown-mark-hidden' }).range(node.from, node.to),
            )
          }
        }
      },
    })
    appendTextPreviewDecorations(view, visible.from, visible.to, imageContext, alwaysPreview, ranges)
  }
  return Decoration.set(ranges, true)
}

function previewRanges(view: EditorView) {
  const ranges: Array<{ from: number; to: number }> = []
  for (const visible of view.visibleRanges) {
    const first = view.state.doc.lineAt(visible.from)
    const last = view.state.doc.lineAt(visible.to)
    const from = view.state.doc.line(Math.max(1, first.number - 20)).from
    const to = view.state.doc.line(Math.min(view.state.doc.lines, last.number + 20)).to
    const previous = ranges.at(-1)
    if (previous && from <= previous.to) previous.to = Math.max(previous.to, to)
    else ranges.push({ from, to })
  }
  return ranges
}

function appendTextPreviewDecorations(
  view: EditorView,
  rangeFrom: number,
  rangeTo: number,
  imageContext: MarkdownImageContext,
  alwaysPreview: boolean,
  ranges: Range<Decoration>[],
) {
  const cursor = view.state.selection.main.head
  const text = view.state.doc.sliceString(rangeFrom, rangeTo)
  const absolute = (match: RegExpMatchArray) => rangeFrom + (match.index ?? 0)
  const outsideCursor = (from: number, to: number) => alwaysPreview || cursor < from || cursor > to
  const footnotes = footnoteDefinitions(text)

  for (const match of text.matchAll(/\[\^([^\]\r\n]+)\]/g)) {
    const from = absolute(match)
    const to = from + match[0].length
    const line = view.state.doc.lineAt(from)
    const before = view.state.doc.sliceString(line.from, from)
    if (!before.trim() && view.state.doc.sliceString(to, to + 1) === ':') continue
    if (outsideCursor(from, to)) {
      const label = match[1] ?? ''
      ranges.push(Decoration.replace({
        widget: new FootnoteReferenceWidget(label, footnotes.get(label) ?? label),
      }).range(from, to))
    }
  }
  for (const match of text.matchAll(/(?<![\\$])\$([^$\n]+?)(?<![\\$])\$(?!\$)/g)) {
    const from = absolute(match)
    const to = from + match[0].length
    if (outsideCursor(from, to)) ranges.push(Decoration.replace({ widget: new MathWidget(match[1] ?? '', false) }).range(from, to))
  }
  for (const match of text.matchAll(/!\[([^\]]*)\]\(([^)\s]+)(?:\s+["'][^"']*["'])?\)/g)) {
    const from = absolute(match)
    const to = from + match[0].length
    if (outsideCursor(from, to)) {
      ranges.push(Decoration.replace({
        widget: new ImageWidget(resolveImageSource(match[2] ?? '', imageContext), match[1] ?? '', imageContext.loadWorkspaceImage),
      }).range(from, to))
    }
  }
  for (const match of text.matchAll(/^(\s*[-*+]\s+)(\[([ xX])\])/gm)) {
    const from = absolute(match) + (match[1]?.length ?? 0)
    const marker = match[2] ?? '[ ]'
    const to = from + marker.length
    const line = view.state.doc.lineAt(from)
    if (alwaysPreview || cursor < line.from || cursor > line.to) {
      ranges.push(Decoration.replace({ widget: new TaskWidget((match[3] ?? '').toLowerCase() === 'x', from, to) }).range(from, to))
    }
  }
}

function blockMarkdownDecorations(
  view: EditorView,
  alwaysPreview: boolean,
): DecorationSet {
  const ranges: Range<Decoration>[] = []
  const cursor = view.state.selection.main.head
  for (const visible of previewRanges(view)) {
    const text = view.state.doc.sliceString(visible.from, visible.to)
    const outsideCursor = (from: number, to: number) => alwaysPreview || cursor < from || cursor > to
    for (const match of text.matchAll(/^\[\^([^\]\r\n]+)\]:[ \t]*(.*(?:\r?\n(?: {2,}|\t).*)*)/gm)) {
      const from = visible.from + (match.index ?? 0)
      const to = from + match[0].length
      if (outsideCursor(from, to)) ranges.push(Decoration.replace({
        widget: new FootnoteDefinitionWidget(match[1] ?? '', (match[2] ?? '').replace(/\r?\n(?: {2,}|\t)/g, '\n')),
        block: true,
      }).range(from, to))
    }
    if (visible.from === 0) {
      const frontmatter = /^(---\s*\r?\n[\s\S]*?\r?\n---)\s*(?:\r?\n|$)/.exec(text)
      if (frontmatter) {
        const to = frontmatter[1]?.length ?? 0
        if (outsideCursor(0, to)) ranges.push(Decoration.replace({
          widget: new FrontmatterWidget(frontmatter[1] ?? ''),
          block: true,
        }).range(0, to))
      }
    }
    appendBlockMatches(text, visible.from, cursor, alwaysPreview, ranges)
  }
  return Decoration.set(ranges, true)
}

function appendBlockMatches(text: string, rangeFrom: number, cursor: number, alwaysPreview: boolean, ranges: Range<Decoration>[]) {
  const append = (pattern: RegExp, widget: (match: RegExpMatchArray) => MermaidWidget | MathWidget | TableWidget) => {
    for (const match of text.matchAll(pattern)) {
      const from = rangeFrom + (match.index ?? 0)
      const to = from + match[0].trimEnd().length
      if (alwaysPreview || cursor < from || cursor > to) ranges.push(Decoration.replace({
        widget: widget(match),
        block: true,
      }).range(from, to))
    }
  }
  append(/^```mermaid[^\n]*\r?\n([\s\S]*?)^```\s*$/gm, (match) => new MermaidWidget(match[1] ?? ''))
  append(/^\$\$\s*\r?\n([\s\S]*?)^\$\$\s*$/gm, (match) => new MathWidget((match[1] ?? '').trim(), true))
  append(/^(?:\|?[^\n|]+\|[^\n]*\r?\n)\|?\s*:?-{3,}:?\s*(?:\|\s*:?-{3,}:?\s*)+\|?\r?\n(?:\|?[^\n]+\|[^\n]*(?:\r?\n|$))+/gm, (match) => new TableWidget(match[0]))
}

/* Block replacements have to be owned by a StateField in CodeMirror. The
 * visible-range plugin below publishes the newest bounded set through this
 * effect, avoiding whole-document block scans on every transaction. */
const setBlockDecorations = StateEffect.define<DecorationSet>()

function blockDecorationField() {
  return StateField.define<DecorationSet>({
    create: () => Decoration.none,
    update(value, transaction) {
      let next = value.map(transaction.changes)
      for (const effect of transaction.effects) {
        if (effect.is(setBlockDecorations)) next = effect.value
      }
      return next
    },
    provide: (field) => EditorView.decorations.from(field),
  })
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
  alwaysPreview = false,
): Extension[] {
  const blocks = blockDecorationField()
  let blockGeneration = 0
  const scheduleBlocks = (view: EditorView) => {
    const generation = ++blockGeneration
    queueMicrotask(() => {
      if (generation !== blockGeneration || !view.dom.isConnected) return
      view.dispatch({
        effects: setBlockDecorations.of(blockMarkdownDecorations(view, alwaysPreview)),
      })
    })
  }
  const liveMarkdown = ViewPlugin.fromClass(
    class {
      decorations: DecorationSet

      constructor(view: EditorView) {
        this.decorations = liveMarkdownDecorations(view, imageContext, alwaysPreview)
        scheduleBlocks(view)
      }

      update(update: ViewUpdate) {
        if (update.docChanged || update.selectionSet || update.viewportChanged) {
          this.decorations = liveMarkdownDecorations(update.view, imageContext, alwaysPreview)
          scheduleBlocks(update.view)
        }
      }

      destroy() {
        blockGeneration += 1
      }
    },
    {
      decorations: (plugin) => plugin.decorations,
    },
  )
  return [blocks, liveMarkdown]
}
