import { ref } from 'vue'

export interface MarkdownSnippet {
  id: string
  name: string
  shortcut: string
  body: string
}

const STORAGE_KEY = 'marktree-markdown-snippets-v1'
const EXPORT_VERSION = 1
const DEFAULT_SNIPPETS: MarkdownSnippet[] = [
  {
    id: 'default-link',
    name: 'Link',
    shortcut: 'link',
    body: '[{{selection}}{{cursor}}](https://)',
  },
  {
    id: 'default-table',
    name: 'Table',
    shortcut: 'table',
    body: '| Column 1 | Column 2 |\n| --- | --- |\n| {{selection}}{{cursor}} |  |',
  },
  {
    id: 'default-code-block',
    name: 'Code block',
    shortcut: 'codeblock',
    body: '```\n{{selection}}{{cursor}}\n```',
  },
  {
    id: 'default-footnote',
    name: 'Footnote',
    shortcut: 'footnote',
    body: '[^1]\n\n[^1]: {{selection}}{{cursor}}',
  },
]

function validSnippet(value: unknown): value is MarkdownSnippet {
  if (!value || typeof value !== 'object') return false
  const snippet = value as Record<string, unknown>
  return (
    typeof snippet.id === 'string' &&
    typeof snippet.name === 'string' &&
    typeof snippet.shortcut === 'string' &&
    typeof snippet.body === 'string'
  )
}

function loadSnippets() {
  try {
    const stored = localStorage.getItem(STORAGE_KEY)
    if (stored === null) return DEFAULT_SNIPPETS.map((snippet) => ({ ...snippet }))
    const value = JSON.parse(stored) as unknown
    if (Array.isArray(value)) return normalizeSnippetList(value)
    if (value && typeof value === 'object') {
      const record = value as Record<string, unknown>
      if (record.version === EXPORT_VERSION && Array.isArray(record.snippets)) {
        return normalizeSnippetList(record.snippets)
      }
    }
    return []
  } catch {
    return []
  }
}

export const markdownSnippets = ref<MarkdownSnippet[]>(loadSnippets())

export function saveMarkdownSnippet(snippet: MarkdownSnippet) {
  const normalized = normalizeSnippet(snippet)
  const index = markdownSnippets.value.findIndex((item) => item.id === normalized.id)
  if (index < 0) markdownSnippets.value.push(normalized)
  else markdownSnippets.value[index] = normalized
  persistSnippets()
}

export function removeMarkdownSnippet(id: string) {
  markdownSnippets.value = markdownSnippets.value.filter((snippet) => snippet.id !== id)
  persistSnippets()
}

export function newMarkdownSnippet(): MarkdownSnippet {
  return {
    id: crypto.randomUUID(),
    name: '',
    shortcut: '',
    body: '{{selection}}{{cursor}}',
  }
}

export function renderMarkdownSnippet(body: string, selection: string) {
  const selected = body.replaceAll('{{selection}}', selection)
  const cursor = selected.indexOf('{{cursor}}')
  const text = selected.replaceAll('{{cursor}}', '')
  return { text, cursor: cursor < 0 ? text.length : cursor }
}

export function serializeMarkdownSnippets() {
  return JSON.stringify(
    { version: EXPORT_VERSION, snippets: markdownSnippets.value },
    null,
    2,
  )
}

export function importMarkdownSnippets(serialized: string, replace = false) {
  const value = JSON.parse(serialized) as unknown
  const raw = Array.isArray(value)
    ? value
    : value &&
        typeof value === 'object' &&
        (value as Record<string, unknown>).version === EXPORT_VERSION &&
        Array.isArray((value as Record<string, unknown>).snippets)
      ? ((value as Record<string, unknown>).snippets as unknown[])
      : undefined
  if (!raw) throw new Error('Invalid Marktree snippet file.')
  const imported = normalizeSnippetList(raw)
  if (!imported.length && raw.length) throw new Error('The snippet file has no valid entries.')
  const next = replace ? imported : mergeSnippets(markdownSnippets.value, imported)
  markdownSnippets.value = next
  persistSnippets()
  return imported.length
}

function normalizeSnippetList(values: unknown[]) {
  const normalized = values
    .filter(validSnippet)
    .map(normalizeSnippet)
    .filter((snippet) => snippet.name && snippet.shortcut && snippet.body)
  const byShortcut = new Map<string, MarkdownSnippet>()
  for (const snippet of normalized) byShortcut.set(snippet.shortcut.toLowerCase(), snippet)
  return [...byShortcut.values()]
}

function normalizeSnippet(snippet: MarkdownSnippet): MarkdownSnippet {
  return {
    ...snippet,
    id: snippet.id.trim() || crypto.randomUUID(),
    name: snippet.name.trim(),
    shortcut: snippet.shortcut
      .trim()
      .replace(/^\/+/, '')
      .replace(/\s+/g, '-')
      .toLowerCase(),
  }
}

function mergeSnippets(current: MarkdownSnippet[], imported: MarkdownSnippet[]) {
  const merged = new Map(current.map((snippet) => [snippet.shortcut.toLowerCase(), snippet]))
  for (const snippet of imported) merged.set(snippet.shortcut.toLowerCase(), snippet)
  return [...merged.values()]
}

function persistSnippets() {
  localStorage.setItem(
    STORAGE_KEY,
    JSON.stringify({ version: EXPORT_VERSION, snippets: markdownSnippets.value }),
  )
}
