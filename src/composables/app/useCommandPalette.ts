import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch, type Ref } from 'vue'
import { useI18n } from 'vue-i18n'

import type { WorkspaceApi } from '@/composables/useWorkspace'
import { isTauri, nativeApi } from '@/lib/api'
import type {
  CommandPaletteAction,
  CommandPaletteItem,
  WorktreeSearchResult,
} from '@/types'

type Workspace = Pick<
  WorkspaceApi,
  | 'activeRoot'
  | 'activeWorkspace'
  | 'activeWorktree'
  | 'activateWorkspace'
  | 'favoriteDocuments'
  | 'notify'
  | 'openSearchResult'
  | 'recentDocuments'
  | 'reportError'
  | 'workspaces'
>

export function useCommandPalette(
  workspace: Workspace,
  options: {
    nativeAndroid: Ref<boolean>
    executeAction: (action: CommandPaletteAction) => void | Promise<void>
    closeModal: () => void
    focusResult: (line: number, column: number) => void
  },
) {
  const { t } = useI18n()
  const open = ref(false)
  const query = ref('')
  const pathPrefix = ref('')
  const fileKind = ref<'all' | 'markdown' | 'text'>('all')
  const modifiedDays = ref(0)
  const searchResults = ref<WorktreeSearchResult[]>([])
  const searching = ref(false)
  let searchGeneration = 0
  let searchTimer: ReturnType<typeof setTimeout> | undefined

  const definitions = computed<
    Array<{ action: CommandPaletteAction; title: string; detail: string }>
  >(() => [
    {
      action: 'addWorkspace',
      title: options.nativeAndroid.value ? t('app.newMobileWorkspace') : t('app.openFolder'),
      detail: options.nativeAndroid.value
        ? t('app.commandNewMobileWorkspaceHint')
        : t('app.commandOpenFolderHint'),
    },
    ...(workspace.activeWorkspace.value
      ? [
          {
            action: 'newDocument' as const,
            title: t('app.newDocument'),
            detail: t('app.commandNewDocumentHint'),
          },
          {
            action: 'newFolder' as const,
            title: t('app.newFolder'),
            detail: t('app.commandNewFolderHint'),
          },
          {
            action: 'refresh' as const,
            title: t('app.refresh'),
            detail: t('app.commandRefreshHint'),
          },
          {
            action: 'settings' as const,
            title: t('app.workspaceSettings'),
            detail: t('app.commandSettingsHint'),
          },
          ...(workspace.activeWorkspace.value.git
            ? [
                {
                  action: 'sync' as const,
                  title: t('app.sync'),
                  detail: t('app.commandSyncHint'),
                },
              ]
            : []),
          {
            action: 'snippets' as const,
            title: t('app.snippets'),
            detail: t('app.commandSnippetsHint'),
          },
        ]
      : []),
  ])

  const results = computed<CommandPaletteItem[]>(() => {
    const rawQuery = query.value.trim()
    const commandOnly = rawQuery.startsWith('>')
    const needle = (commandOnly ? rawQuery.slice(1) : rawQuery).trim().toLowerCase()
    const commands = definitions.value
      .filter(
        (command) =>
          !needle ||
          command.action.toLowerCase().includes(needle) ||
          command.title.toLowerCase().includes(needle) ||
          command.detail.toLowerCase().includes(needle),
      )
      .map((command) => ({
        key: `command:${command.action}`,
        kind: 'command' as const,
        ...command,
      }))
    if (commandOnly) return commands
    const workspaceItems = workspace.workspaces.value
      .flatMap((candidate) => [
        {
          key: `workspace:${candidate.id}`,
          kind: 'workspace' as const,
          title: candidate.name,
          detail: `${candidate.git ? t('app.gitWorkspace') : t('app.localWorkspace')} · ${candidate.root}`,
          workspaceId: candidate.id,
        },
        ...(candidate.git?.worktrees.map((worktree) => ({
          key: `worktree:${candidate.id}:${worktree.path}`,
          kind: 'workspace' as const,
          title: `${candidate.name} · ${worktree.name}`,
          detail: `${worktree.branch ?? t('app.detachedHead')} · ${worktree.path}`,
          workspaceId: candidate.id,
          worktreeRoot: worktree.path,
        })) ?? []),
      ])
      .filter(
        (candidate) =>
          !needle ||
          candidate.title.toLowerCase().includes(needle) ||
          candidate.detail.toLowerCase().includes(needle),
      )
    const documents = rawQuery
      ? searchResults.value
      : [...workspace.favoriteDocuments.value, ...workspace.recentDocuments.value]
          .filter(
            (document, index, all) =>
              all.findIndex((candidate) => candidate.path === document.path) === index,
          )
          .slice(0, 30)
          .map((document) => ({
            worktree:
              workspace.activeWorktree.value?.name ??
              workspace.activeWorkspace.value?.name ??
              '',
            root: workspace.activeRoot.value ?? '',
            path: document.path,
            line: null,
            column: null,
            snippet: document.path,
            matchType: 'path' as const,
            fileKind: document.fileKind ?? 'text',
            modifiedMs: document.modifiedMs,
          }))
    return [
      ...commands,
      ...workspaceItems,
      ...documents.map((result) => ({
        key: `document:${result.root}:${result.path}:${result.line ?? 0}:${result.column ?? 0}:${result.matchType}`,
        kind: 'document' as const,
        title: result.path.split('/').at(-1) ?? result.path,
        detail:
          result.matchType === 'content' && result.line
            ? `${result.worktree} · ${result.path}:${result.line} · ${result.snippet}`
            : `${result.worktree} · ${result.path}`,
        result,
      })),
    ]
  })

  function show(commandOnly = false) {
    query.value = commandOnly ? '>' : ''
    searchResults.value = []
    open.value = true
  }

  async function choose(item: CommandPaletteItem) {
    open.value = false
    if (item.kind === 'command') {
      await options.executeAction(item.action)
      return
    }
    if (item.kind === 'workspace') {
      await workspace.activateWorkspace(item.workspaceId, item.worktreeRoot)
      return
    }
    await workspace.openSearchResult(item.result)
    await nextTick()
    if (item.result.line) {
      options.focusResult(item.result.line, item.result.column ?? 1)
    }
  }

  async function searchDocuments(value: string, generation: number) {
    const root = workspace.activeRoot.value
    const owner = workspace.activeWorkspace.value
    if (!root || !owner || !isTauri()) {
      searchResults.value = workspace.recentDocuments.value
        .filter((document) => document.path.toLowerCase().includes(value.toLowerCase()))
        .map((document) => ({
          worktree: workspace.activeWorktree.value?.name ?? owner?.name ?? '',
          root: root ?? '',
          path: document.path,
          line: null,
          column: null,
          snippet: document.path,
          matchType: 'path',
          fileKind: document.fileKind ?? 'text',
          modifiedMs: document.modifiedMs,
        }))
      searching.value = false
      return
    }
    try {
      const modifiedAfterMs = modifiedDays.value
        ? Date.now() - modifiedDays.value * 24 * 60 * 60 * 1000
        : null
      const fileKinds = fileKind.value === 'all' ? [] : [fileKind.value]
      const candidates = [
        owner,
        ...workspace.workspaces.value.filter((candidate) => candidate.id !== owner.id),
      ]
      const results: WorktreeSearchResult[] = []
      let truncated = false
      for (const candidate of candidates) {
        if (generation !== searchGeneration || query.value.trim() !== value) return
        const remaining = 100 - results.length
        if (remaining <= 0) break
        if (candidate.git) {
          const response = await nativeApi.searchWorktrees({
              request: {
                root: candidate.root,
                query: value,
                limit: remaining,
                pathPrefix: pathPrefix.value || null,
                fileKinds,
                modifiedAfterMs,
              },
            })
          results.push(...response.results)
          truncated ||= response.statistics.truncated
        } else {
          const response = await nativeApi.searchDocuments({
            request: {
              root: candidate.root,
              query: value,
              limit: remaining,
              pathPrefix: pathPrefix.value || null,
              fileKinds,
              modifiedAfterMs,
            },
          })
          results.push(...response.results.map((result) => ({
              worktree: candidate.name,
              root: candidate.root,
              ...result,
            })))
          truncated ||= response.statistics.truncated
        }
      }
      if (generation !== searchGeneration || query.value.trim() !== value) return
      searchResults.value = results.slice(0, 100)
      if (truncated) {
        workspace.notify(t('app.searchLimited'))
      }
    } catch (reason) {
      workspace.reportError(reason)
    } finally {
      if (generation === searchGeneration) searching.value = false
    }
  }

  function handleShortcut(event: KeyboardEvent) {
    if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 'p') {
      event.preventDefault()
      show(event.shiftKey)
    }
    if (event.key === 'Escape') {
      open.value = false
      options.closeModal()
    }
  }

  onMounted(() => window.addEventListener('keydown', handleShortcut))
  onBeforeUnmount(() => {
    window.removeEventListener('keydown', handleShortcut)
    if (searchTimer) clearTimeout(searchTimer)
  })
  watch([query, pathPrefix, fileKind, modifiedDays], ([value]) => {
    if (searchTimer) clearTimeout(searchTimer)
    const normalized = value.trim()
    if (!open.value || !normalized || normalized.startsWith('>')) {
      searchResults.value = []
      searching.value = false
      return
    }
    const generation = ++searchGeneration
    searchResults.value = []
    if (isTauri()) {
      void nativeApi.cancelSearches({
        roots: workspace.workspaces.value.map((candidate) => candidate.root),
      }).catch(() => undefined)
    }
    searching.value = true
    searchTimer = setTimeout(() => void searchDocuments(normalized, generation), 120)
  })

  return {
    open,
    query,
    pathPrefix,
    fileKind,
    modifiedDays,
    searching,
    results,
    show,
    choose,
  }
}
