import { ref } from 'vue'

interface PersistedWorkspaceSession {
  tabs: string[]
  active?: string
  expanded?: string[]
}

interface PersistedWorkspaceUiState {
  version: 1
  sessions: Record<string, PersistedWorkspaceSession>
  favorites: string[]
}

const STORAGE_KEY = 'marktree-workspace-ui-v1'

function emptyState(): PersistedWorkspaceUiState {
  return { version: 1, sessions: {}, favorites: [] }
}

function validPathList(value: unknown): value is string[] {
  return Array.isArray(value) && value.every((item) => typeof item === 'string')
}

function loadState(): PersistedWorkspaceUiState {
  try {
    const value = JSON.parse(localStorage.getItem(STORAGE_KEY) ?? 'null') as unknown
    if (!value || typeof value !== 'object') return emptyState()
    const record = value as Record<string, unknown>
    if (record.version !== 1 || !record.sessions || typeof record.sessions !== 'object') {
      return emptyState()
    }
    const sessions: Record<string, PersistedWorkspaceSession> = {}
    for (const [root, rawSession] of Object.entries(
      record.sessions as Record<string, unknown>,
    )) {
      if (!rawSession || typeof rawSession !== 'object') continue
      const session = rawSession as Record<string, unknown>
      if (!validPathList(session.tabs)) continue
      sessions[root] = {
        tabs: [...new Set(session.tabs)],
        active: typeof session.active === 'string' ? session.active : undefined,
        expanded: validPathList(session.expanded)
          ? [...new Set(session.expanded)]
          : [],
      }
    }
    return {
      version: 1,
      sessions,
      favorites: validPathList(record.favorites)
        ? [...new Set(record.favorites)]
        : [],
    }
  } catch {
    return emptyState()
  }
}

const persisted = loadState()
export const favoriteDocumentKeys = ref<string[]>(persisted.favorites)

function persist() {
  persisted.favorites = favoriteDocumentKeys.value
  localStorage.setItem(STORAGE_KEY, JSON.stringify(persisted))
}

export function restoredWorkspaceSession(root: string) {
  const session = persisted.sessions[root]
  return session
    ? {
        tabs: [...session.tabs],
        active: session.active,
        expanded: [...(session.expanded ?? [])],
      }
    : undefined
}

export function saveWorkspaceSession(root: string, tabs: string[], active?: string) {
  persisted.sessions[root] = {
    tabs: [...new Set(tabs)],
    active: active && tabs.includes(active) ? active : undefined,
    expanded: persisted.sessions[root]?.expanded ?? [],
  }
  persist()
}

export function saveWorkspaceExpansion(root: string, expanded: Iterable<string>) {
  const session = persisted.sessions[root] ?? { tabs: [] }
  session.expanded = [...new Set(expanded)]
  persisted.sessions[root] = session
  persist()
}

export function restoredWorkspaceExpansion(root: string) {
  return new Set(persisted.sessions[root]?.expanded ?? [])
}

export function removeWorkspaceUiState(roots: Iterable<string>) {
  const removed = new Set(roots)
  for (const root of removed) delete persisted.sessions[root]
  favoriteDocumentKeys.value = favoriteDocumentKeys.value.filter(
    (key) => !removed.has(splitDocumentKey(key).root),
  )
  persist()
}

export function isFavoriteDocument(root: string, path: string) {
  return favoriteDocumentKeys.value.includes(documentKey(root, path))
}

export function toggleFavoriteDocument(root: string, path: string) {
  const key = documentKey(root, path)
  favoriteDocumentKeys.value = favoriteDocumentKeys.value.includes(key)
    ? favoriteDocumentKeys.value.filter((candidate) => candidate !== key)
    : [key, ...favoriteDocumentKeys.value]
  persist()
}

export function removeFavoritePaths(root: string, parentPath: string) {
  favoriteDocumentKeys.value = favoriteDocumentKeys.value.filter((key) => {
    const document = splitDocumentKey(key)
    return (
      document.root !== root ||
      (document.path !== parentPath && !document.path.startsWith(`${parentPath}/`))
    )
  })
  persist()
}

export function migrateWorkspacePaths(
  root: string,
  moves: Array<{ oldPath: string; newPath: string }>,
) {
  const byOldPath = new Map(moves.map((move) => [move.oldPath, move.newPath]))
  const session = persisted.sessions[root]
  if (session) {
    session.tabs = session.tabs.map((path) => byOldPath.get(path) ?? path)
    if (session.active) session.active = byOldPath.get(session.active) ?? session.active
  }
  favoriteDocumentKeys.value = favoriteDocumentKeys.value.map((key) => {
    const document = splitDocumentKey(key)
    const next = document.root === root ? byOldPath.get(document.path) : undefined
    return next ? documentKey(root, next) : key
  })
  persist()
}

export function documentKey(root: string, path: string) {
  return `${root}\n${path}`
}

export function splitDocumentKey(key: string) {
  const separator = key.indexOf('\n')
  return separator < 0
    ? { root: '', path: key }
    : { root: key.slice(0, separator), path: key.slice(separator + 1) }
}
