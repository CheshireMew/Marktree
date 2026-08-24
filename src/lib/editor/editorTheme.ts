import type { Extension } from '@codemirror/state'
import { EditorView } from '@codemirror/view'

export function editorTheme(dark: boolean): Extension {
  return EditorView.theme(
    {
      '&': {
        height: '100%',
        backgroundColor: 'transparent',
        color: 'var(--editor-text)',
        fontSize: 'var(--editor-font-size, 17px)',
      },
      '.cm-scroller': {
        fontFamily: 'var(--editor-font-family, var(--editor-font))',
        lineHeight: 'var(--editor-line-height, 1.82)',
        overflow: 'auto',
      },
      '.cm-content': {
        maxWidth: '860px',
        margin: '0 auto',
        padding: '56px 72px 35vh',
        caretColor: 'var(--accent)',
      },
      '.cm-focused': { outline: 'none' },
      '.cm-line': { padding: '0 2px' },
      '.cm-cursor': { borderLeftColor: 'var(--accent)', borderLeftWidth: '2px' },
      '.cm-selectionBackground, ::selection': {
        backgroundColor: dark ? '#33594f' : '#cfe2d9',
      },
      '.cm-gutters': { display: 'none' },
      '.cm-activeLine': { backgroundColor: 'transparent' },
      '.cm-tooltip': {
        background: 'var(--panel)',
        color: 'var(--text)',
        border: '1px solid var(--border)',
      },
    },
    { dark },
  )
}
