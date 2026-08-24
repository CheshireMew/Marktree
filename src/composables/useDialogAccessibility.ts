import { nextTick, onBeforeUnmount, onMounted, watch, type Ref } from 'vue'

type DialogIdentity = string | number | boolean | object | null | undefined

interface DialogOptions {
  closeOnEscape?: boolean | Ref<boolean>
}

interface ActiveDialog {
  surface: Ref<HTMLElement | undefined>
}

const dialogStack: ActiveDialog[] = []

function optionValue(value: boolean | Ref<boolean> | undefined, fallback: boolean) {
  if (value === undefined) return fallback
  return typeof value === 'boolean' ? value : value.value
}

function focusableElements(surface: HTMLElement) {
  return [...surface.querySelectorAll<HTMLElement>(
    'a[href], button:not([disabled]), input:not([disabled]):not([type="hidden"]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])',
  )].filter((element) => !element.hidden && element.getAttribute('aria-hidden') !== 'true')
}

export function useDialogAccessibility(
  active: Ref<DialogIdentity>,
  surface: Ref<HTMLElement | undefined>,
  close: () => void,
  options: DialogOptions = {},
) {
  const instance: ActiveDialog = { surface }
  let previouslyFocused: HTMLElement | null = null

  function removeFromStack() {
    const index = dialogStack.indexOf(instance)
    if (index >= 0) dialogStack.splice(index, 1)
  }

  async function focusInitial() {
    await nextTick()
    if (!active.value || dialogStack.at(-1) !== instance) return
    const dialog = surface.value
    if (!dialog) return
    const target =
      dialog.querySelector<HTMLElement>('[data-dialog-initial-focus]') ??
      dialog.querySelector<HTMLElement>('[autofocus]') ??
      focusableElements(dialog)[0] ??
      dialog
    target.focus()
  }

  function activate(captureFocus: boolean) {
    if (captureFocus) {
      previouslyFocused =
        document.activeElement instanceof HTMLElement ? document.activeElement : null
    }
    removeFromStack()
    dialogStack.push(instance)
    void focusInitial()
  }

  function deactivate(restoreFocus: boolean) {
    removeFromStack()
    if (restoreFocus && previouslyFocused?.isConnected) {
      const target = previouslyFocused
      void nextTick(() => target.focus())
    }
    previouslyFocused = null
  }

  function onKeydown(event: KeyboardEvent) {
    if (!active.value || dialogStack.at(-1) !== instance) return
    const dialog = surface.value
    if (!dialog) return
    if (event.key === 'Escape' && optionValue(options.closeOnEscape, true)) {
      event.preventDefault()
      event.stopPropagation()
      close()
      return
    }
    if (event.key !== 'Tab') return
    const focusable = focusableElements(dialog)
    if (!focusable.length) {
      event.preventDefault()
      dialog.focus()
      return
    }
    const first = focusable[0]
    const last = focusable.at(-1)!
    const current = document.activeElement
    if (!dialog.contains(current)) {
      event.preventDefault()
      ;(event.shiftKey ? last : first).focus()
    } else if (event.shiftKey && current === first) {
      event.preventDefault()
      last.focus()
    } else if (!event.shiftKey && current === last) {
      event.preventDefault()
      first.focus()
    }
  }

  watch(
    active,
    (value, previous) => {
      if (value) activate(!previous)
      else if (previous) deactivate(true)
    },
    { immediate: true, flush: 'post' },
  )
  onMounted(() => document.addEventListener('keydown', onKeydown, true))
  onBeforeUnmount(() => {
    document.removeEventListener('keydown', onKeydown, true)
    deactivate(true)
  })
}
