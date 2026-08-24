import { afterEach, describe, expect, it } from 'vitest'

import { i18n } from '../src/i18n'
import { readableError } from '../src/lib/errors'
import {
  error,
  message,
  setError,
  setNotice,
} from '../src/composables/workspace/feedback'

afterEach(() => {
  i18n.global.locale.value = 'en'
  error.value = ''
  message.value = ''
})

describe('readable native errors', () => {
  it('shows only the newest global outcome instead of mixing success and failure', () => {
    setError('Something failed')
    expect(error.value).toContain('Something failed')
    expect(message.value).toBe('')

    setNotice('Retry succeeded')
    expect(message.value).toBe('Retry succeeded')
    expect(error.value).toBe('')
  })

  it('turns a native error code into an actionable localized message', () => {
    i18n.global.locale.value = 'en'
    const message = readableError({
      code: 'fileNotFound',
      message: 'File not found: notes/missing.md',
    })

    expect(message).toContain('Refresh the workspace')
    expect(message).toContain('Technical detail: File not found: notes/missing.md')
  })

  it('does not expose redundant implementation text for external-change choices', () => {
    i18n.global.locale.value = 'zh-CN'
    const message = readableError({
      code: 'externalChange',
      message: 'The file changed outside Marktree.',
    })

    expect(message).toContain('请选择保留磁盘版本或编辑器版本')
    expect(message).not.toContain('outside Marktree')
  })
})
