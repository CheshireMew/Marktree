import { EditorView, WidgetType } from '@codemirror/view'
import katex from 'katex'

import { i18n } from '@/i18n'
import type { WorkspaceImageLoader } from '@/types'

export type ResolvedImageSource =
  | { kind: 'external'; key: string; url: string }
  | { kind: 'workspace'; key: string; root: string; path: string }

let mermaidLoader: Promise<typeof import('mermaid')> | undefined

function loadMermaid() {
  mermaidLoader ??= import('mermaid').then((module) => {
    module.default.initialize({
      startOnLoad: false,
      securityLevel: 'strict',
      theme: 'neutral',
      fontFamily: 'Inter, "Microsoft YaHei", sans-serif',
    })
    return module
  })
  return mermaidLoader
}

export class MathWidget extends WidgetType {
  constructor(private readonly source: string, private readonly displayMode: boolean) {
    super()
  }

  eq(other: MathWidget) {
    return other.source === this.source && other.displayMode === this.displayMode
  }

  toDOM() {
    const element = document.createElement(this.displayMode ? 'div' : 'span')
    element.className = this.displayMode ? 'cm-math-block' : 'cm-math-inline'
    try {
      katex.render(this.source, element, {
        displayMode: this.displayMode,
        throwOnError: false,
        strict: false,
      })
    } catch {
      element.textContent = this.source
    }
    return element
  }
}

export class MermaidWidget extends WidgetType {
  constructor(private readonly source: string) {
    super()
  }

  eq(other: MermaidWidget) {
    return other.source === this.source
  }

  toDOM() {
    const element = document.createElement('div')
    element.className = 'cm-mermaid'
    element.textContent = i18n.global.t('app.renderingDiagram')
    const id = `marktree-mermaid-${crypto.randomUUID().replaceAll('-', '')}`
    void loadMermaid()
      .then((module) => module.default.render(id, this.source))
      .then(({ svg }) => {
        element.innerHTML = svg
      })
      .catch(() => {
        const fallback = document.createElement('pre')
        fallback.textContent = this.source
        element.replaceChildren(fallback)
      })
    return element
  }
}

export class ImageWidget extends WidgetType {
  constructor(
    private readonly source: ResolvedImageSource,
    private readonly alt: string,
    private readonly loadWorkspaceImage: WorkspaceImageLoader,
  ) {
    super()
  }

  eq(other: ImageWidget) {
    return (
      other.source.kind === this.source.kind &&
      other.source.key === this.source.key &&
      other.alt === this.alt
    )
  }

  toDOM() {
    const figure = document.createElement('figure')
    figure.className = 'cm-markdown-image'
    const image = document.createElement('img')
    image.alt = this.alt
    if (this.source.kind === 'external') {
      image.src = this.source.url
    } else {
      const workspaceSource = this.source
      void this.loadWorkspaceImage(workspaceSource.root, workspaceSource.path)
        .then((source) => {
          image.src = source
        })
        .catch(() => {
          figure.classList.add('load-error')
          image.alt = i18n.global.t('app.imageLoadFailed', {
            path: workspaceSource.path,
          })
        })
    }
    figure.append(image)
    if (this.alt) {
      const caption = document.createElement('figcaption')
      caption.textContent = this.alt
      figure.append(caption)
    }
    return figure
  }
}

export class TaskWidget extends WidgetType {
  constructor(
    private readonly checked: boolean,
    private readonly from: number,
    private readonly to: number,
  ) {
    super()
  }

  eq(other: TaskWidget) {
    return (
      other.checked === this.checked && other.from === this.from && other.to === this.to
    )
  }

  toDOM(view: EditorView) {
    const input = document.createElement('input')
    input.className = 'cm-task-checkbox'
    input.type = 'checkbox'
    input.checked = this.checked
    input.addEventListener('change', () => {
      view.dispatch({
        changes: {
          from: this.from,
          to: this.to,
          insert: input.checked ? '[x]' : '[ ]',
        },
      })
    })
    return input
  }

  ignoreEvent() {
    return false
  }
}

export class TableWidget extends WidgetType {
  constructor(private readonly source: string) {
    super()
  }

  eq(other: TableWidget) {
    return other.source === this.source
  }

  toDOM() {
    const wrapper = document.createElement('div')
    wrapper.className = 'cm-markdown-table'
    const rows = this.source
      .trim()
      .split(/\r?\n/)
      .map((row) => row.trim().replace(/^\||\|$/g, '').split('|').map((cell) => cell.trim()))
    const table = document.createElement('table')
    rows.forEach((cells, rowIndex) => {
      if (rowIndex === 1) return
      const row = document.createElement('tr')
      cells.forEach((cell) => {
        const element = document.createElement(rowIndex === 0 ? 'th' : 'td')
        element.textContent = cell
        row.append(element)
      })
      table.append(row)
    })
    wrapper.append(table)
    return wrapper
  }
}

export class FrontmatterWidget extends WidgetType {
  constructor(private readonly source: string) {
    super()
  }

  eq(other: FrontmatterWidget) {
    return other.source === this.source
  }

  toDOM() {
    const container = document.createElement('dl')
    container.className = 'cm-frontmatter'
    for (const line of this.source.split(/\r?\n/).slice(1, -1)) {
      const separator = line.indexOf(':')
      if (separator < 0) continue
      const key = document.createElement('dt')
      const value = document.createElement('dd')
      key.textContent = line.slice(0, separator).trim()
      value.textContent = line.slice(separator + 1).trim()
      container.append(key, value)
    }
    return container
  }
}

export class FootnoteReferenceWidget extends WidgetType {
  constructor(
    private readonly label: string,
    private readonly content: string,
  ) {
    super()
  }

  eq(other: FootnoteReferenceWidget) {
    return other.label === this.label && other.content === this.content
  }

  toDOM() {
    const reference = document.createElement('sup')
    reference.className = 'cm-footnote-reference'
    reference.textContent = this.label
    reference.title = this.content
    reference.setAttribute('aria-label', `${this.label}: ${this.content}`)
    return reference
  }
}

export class FootnoteDefinitionWidget extends WidgetType {
  constructor(
    private readonly label: string,
    private readonly content: string,
  ) {
    super()
  }

  eq(other: FootnoteDefinitionWidget) {
    return other.label === this.label && other.content === this.content
  }

  toDOM() {
    const note = document.createElement('aside')
    note.className = 'cm-footnote-definition'
    const label = document.createElement('sup')
    const content = document.createElement('span')
    label.textContent = this.label
    content.textContent = this.content
    note.append(label, content)
    return note
  }
}
