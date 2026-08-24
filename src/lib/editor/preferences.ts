import { reactive, watch } from 'vue'

export type EditorFontFamily = 'system' | 'serif' | 'monospace'
export type InterfaceDensity = 'comfortable' | 'compact'

export interface EditorPreferences {
  fontFamily: EditorFontFamily
  fontSize: number
  spellcheck: boolean
  density: InterfaceDensity
  sidebarWidth: number
}

const STORAGE_KEY = 'marktree-editor-preferences-v1'
const DEFAULTS: EditorPreferences = {
  fontFamily: 'system',
  fontSize: 16,
  spellcheck: true,
  density: 'comfortable',
  sidebarWidth: 280,
}

function loadPreferences(): EditorPreferences {
  try {
    const stored = JSON.parse(localStorage.getItem(STORAGE_KEY) ?? '{}') as Partial<EditorPreferences>
    return {
      fontFamily: ['system', 'serif', 'monospace'].includes(stored.fontFamily ?? '')
        ? (stored.fontFamily as EditorFontFamily)
        : DEFAULTS.fontFamily,
      fontSize:
        typeof stored.fontSize === 'number'
          ? Math.min(24, Math.max(12, Math.round(stored.fontSize)))
          : DEFAULTS.fontSize,
      spellcheck:
        typeof stored.spellcheck === 'boolean' ? stored.spellcheck : DEFAULTS.spellcheck,
      density: stored.density === 'compact' ? 'compact' : DEFAULTS.density,
      sidebarWidth:
        typeof stored.sidebarWidth === 'number'
          ? Math.min(440, Math.max(220, Math.round(stored.sidebarWidth)))
          : DEFAULTS.sidebarWidth,
    }
  } catch {
    return { ...DEFAULTS }
  }
}

export const editorPreferences = reactive<EditorPreferences>(loadPreferences())

watch(
  editorPreferences,
  (value) => localStorage.setItem(STORAGE_KEY, JSON.stringify(value)),
  { deep: true, flush: 'sync' },
)

export function resetEditorPreferences() {
  Object.assign(editorPreferences, DEFAULTS)
}

export function editorFontStack(family: EditorFontFamily) {
  if (family === 'serif') return 'Georgia, "Noto Serif", "Microsoft YaHei", serif'
  if (family === 'monospace') return '"Cascadia Code", "JetBrains Mono", Consolas, monospace'
  return 'Inter, "Segoe UI", "Microsoft YaHei", sans-serif'
}
