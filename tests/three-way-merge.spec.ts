import { describe, expect, it } from 'vitest'

import { mergeThreeWay, resolvedMergeContent } from '../src/lib/threeWayMerge'

describe('three-way Markdown merge', () => {
  it('combines independent edits without asking the user', () => {
    const result = mergeThreeWay(
      '# Title\r\n\r\nFirst\r\nSecond\r\n',
      '# Local title\r\n\r\nFirst\r\nSecond\r\n',
      '# Title\r\n\r\nFirst\r\nRemote second\r\n',
    )

    expect(result.conflictCount).toBe(0)
    expect(result.content).toBe('# Local title\r\n\r\nFirst\r\nRemote second\r\n')
  })

  it('exposes only overlapping edits as selectable segments', () => {
    const result = mergeThreeWay(
      '# Title\n\nShared\n',
      '# Title\n\nLocal\n',
      '# Title\n\nRemote\n',
    )

    expect(result.conflictCount).toBe(1)
    const conflict = result.segments.find((segment) => segment.conflicting)!
    expect(conflict.base).toBe('Shared\n')
    expect(conflict.local).toBe('Local\n')
    expect(conflict.remote).toBe('Remote\n')

    conflict.content = conflict.remote
    expect(resolvedMergeContent(result.segments)).toBe('# Title\n\nRemote\n')
  })

  it('treats two different additions to an empty ancestor as one real conflict', () => {
    const result = mergeThreeWay('', 'Local only\n', 'Remote only\n')

    expect(result.conflictCount).toBe(1)
    expect(result.segments[0]?.local).toBe('Local only\n')
    expect(result.segments[0]?.remote).toBe('Remote only\n')
  })
})
