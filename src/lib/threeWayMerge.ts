import { diffArrays } from 'diff'

export interface ThreeWaySegment {
  id: string
  base: string
  local: string
  remote: string
  content: string
  conflicting: boolean
}

export interface ThreeWayMerge {
  segments: ThreeWaySegment[]
  content: string
  conflictCount: number
}

interface Edit {
  start: number
  end: number
  replacement: string[]
  side: 'local' | 'remote'
}

export function mergeThreeWay(baseText: string, localText: string, remoteText: string): ThreeWayMerge {
  const base = lines(baseText)
  const edits = [
    ...editsFromBase(base, lines(localText), 'local'),
    ...editsFromBase(base, lines(remoteText), 'remote'),
  ].sort((left, right) => left.start - right.start || left.end - right.end)
  const segments: ThreeWaySegment[] = []
  let cursor = 0
  let editIndex = 0

  while (editIndex < edits.length) {
    const cluster = [edits[editIndex]!]
    let clusterStart = edits[editIndex]!.start
    let clusterEnd = edits[editIndex]!.end
    editIndex += 1
    while (editIndex < edits.length && belongsToCluster(edits[editIndex]!, clusterStart, clusterEnd)) {
      const next = edits[editIndex]!
      cluster.push(next)
      clusterStart = Math.min(clusterStart, next.start)
      clusterEnd = Math.max(clusterEnd, next.end)
      editIndex += 1
    }

    if (cursor < clusterStart) {
      const content = base.slice(cursor, clusterStart).join('')
      segments.push(unchangedSegment(segments.length, content))
    }

    const localEdits = cluster.filter((edit) => edit.side === 'local')
    const remoteEdits = cluster.filter((edit) => edit.side === 'remote')
    const baseContent = base.slice(clusterStart, clusterEnd).join('')
    const localContent = applyEdits(base, clusterStart, clusterEnd, localEdits)
    const remoteContent = applyEdits(base, clusterStart, clusterEnd, remoteEdits)
    const onlyLocal = remoteEdits.length === 0
    const onlyRemote = localEdits.length === 0
    const identical = localContent === remoteContent
    const content = onlyRemote ? remoteContent : localContent
    segments.push({
      id: `segment-${segments.length}`,
      base: baseContent,
      local: localContent,
      remote: remoteContent,
      content: identical ? localContent : content,
      conflicting: !onlyLocal && !onlyRemote && !identical,
    })
    cursor = clusterEnd
  }

  if (cursor < base.length) {
    segments.push(unchangedSegment(segments.length, base.slice(cursor).join('')))
  }
  if (!segments.length) {
    segments.push(unchangedSegment(0, baseText))
  }

  return {
    content: segments.map((segment) => segment.content).join(''),
    conflictCount: segments.filter((segment) => segment.conflicting).length,
    segments,
  }
}

export function resolvedMergeContent(segments: ThreeWaySegment[]) {
  return segments.map((segment) => segment.content).join('')
}

function lines(text: string) {
  return text.match(/[^\n]*\n|[^\n]+$/g) ?? []
}

function editsFromBase(base: string[], variant: string[], side: Edit['side']) {
  const edits: Edit[] = []
  let cursor = 0
  let start: number | undefined
  let replacement: string[] = []

  const flush = () => {
    if (start === undefined) return
    edits.push({ start, end: cursor, replacement, side })
    start = undefined
    replacement = []
  }

  for (const change of diffArrays(base, variant)) {
    if (!change.added && !change.removed) {
      flush()
      cursor += change.value.length
    } else {
      start ??= cursor
      if (change.removed) cursor += change.value.length
      if (change.added) replacement.push(...change.value)
    }
  }
  flush()
  return edits
}

function belongsToCluster(edit: Edit, clusterStart: number, clusterEnd: number) {
  if (edit.start < clusterEnd) return true
  return clusterStart === clusterEnd && edit.start === clusterStart && edit.end === edit.start
}

function applyEdits(base: string[], start: number, end: number, edits: Edit[]) {
  if (!edits.length) return base.slice(start, end).join('')
  const ordered = [...edits].sort((left, right) => left.start - right.start || left.end - right.end)
  const output: string[] = []
  let cursor = start
  for (const edit of ordered) {
    output.push(...base.slice(cursor, edit.start), ...edit.replacement)
    cursor = edit.end
  }
  output.push(...base.slice(cursor, end))
  return output.join('')
}

function unchangedSegment(index: number, content: string): ThreeWaySegment {
  return {
    id: `segment-${index}`,
    base: content,
    local: content,
    remote: content,
    content,
    conflicting: false,
  }
}
