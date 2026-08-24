import { createApp, defineComponent, h, nextTick, ref } from 'vue'
import { afterEach, describe, expect, it, vi } from 'vitest'

import WorkspaceOverlays from '../src/components/WorkspaceOverlays.vue'
import { useDialogAccessibility } from '../src/composables/useDialogAccessibility'
import { i18n } from '../src/i18n'

const mounted: Array<ReturnType<typeof createApp>> = []

afterEach(() => {
  for (const app of mounted.splice(0)) app.unmount()
  document.body.replaceChildren()
})

describe('dialog accessibility contract', () => {
  it('traps focus, closes the top dialog with Escape, and restores the trigger', async () => {
    const component = defineComponent({
      setup() {
        const open = ref(false)
        const surface = ref<HTMLElement>()
        useDialogAccessibility(open, surface, () => {
          open.value = false
        })
        return () =>
          h('div', [
            h(
              'button',
              { id: 'trigger', onClick: () => (open.value = true) },
              'Open',
            ),
            open.value
              ? h(
                  'section',
                  {
                    ref: surface,
                    role: 'dialog',
                    'aria-modal': 'true',
                    tabindex: -1,
                  },
                  [
                    h('button', { id: 'first', 'data-dialog-initial-focus': '' }, 'First'),
                    h('button', { id: 'last' }, 'Last'),
                  ],
                )
              : null,
          ])
      },
    })
    const host = document.createElement('div')
    document.body.append(host)
    const app = createApp(component)
    app.mount(host)
    mounted.push(app)
    const trigger = host.querySelector<HTMLButtonElement>('#trigger')!
    trigger.focus()
    trigger.click()
    await nextTick()
    await nextTick()

    expect(document.activeElement?.id).toBe('first')
    host.querySelector<HTMLButtonElement>('#last')!.focus()
    document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Tab', bubbles: true }))
    expect(document.activeElement?.id).toBe('first')
    document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }))
    await nextTick()
    await nextTick()
    expect(host.querySelector('[role="dialog"]')).toBeNull()
    expect(document.activeElement).toBe(trigger)
  })

  it('exposes command palette combobox and listbox relationships', async () => {
    const host = document.createElement('div')
    document.body.append(host)
    const app = createApp(WorkspaceOverlays, {
      commandPaletteOpen: true,
      commandPaletteQuery: '',
      commandPaletteResults: [
        {
          key: 'new-document',
          kind: 'command',
          action: 'newDocument',
          title: 'New document',
          detail: 'Create a document',
        },
      ],
      commandPalettePathPrefix: '',
      commandPaletteFileKind: 'all',
      commandPaletteModifiedDays: 0,
      onCloseCommandPalette: vi.fn(),
    })
    app.use(i18n)
    app.mount(host)
    mounted.push(app)
    await nextTick()

    const dialog = host.querySelector<HTMLElement>('[role="dialog"]')!
    const input = host.querySelector<HTMLInputElement>('[role="combobox"]')!
    const listbox = host.querySelector<HTMLElement>('[role="listbox"]')!
    const option = host.querySelector<HTMLElement>('[role="option"]')!
    expect(dialog.getAttribute('aria-modal')).toBe('true')
    expect(input.getAttribute('aria-controls')).toBe(listbox.id)
    expect(input.getAttribute('aria-activedescendant')).toBe(option.id)
    expect(option.getAttribute('aria-selected')).toBe('true')
  })
})
