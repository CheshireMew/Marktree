import type { EditorTab } from '@/types'

import { sessions } from './state'

const pendingByTab = new WeakMap<EditorTab, Set<Promise<void>>>()

export function trackTabUpload(tab: EditorTab, task: Promise<void>) {
  let tasks = pendingByTab.get(tab)
  if (!tasks) {
    tasks = new Set()
    pendingByTab.set(tab, tasks)
  }
  tasks.add(task)
  void task.finally(() => {
    const current = pendingByTab.get(tab)
    current?.delete(task)
    if (!current?.size) pendingByTab.delete(tab)
  }).catch(() => undefined)
  return task
}

export async function waitForTabUploads(tab: EditorTab) {
  while (true) {
    const tasks = [...(pendingByTab.get(tab) ?? [])]
    if (!tasks.length) return
    await Promise.allSettled(tasks)
  }
}

export async function waitForUploadsUnderPath(root: string, path: string) {
  const session = sessions.get(root)
  const tabs = session?.tabs.filter(
    (tab) => tab.path === path || tab.path.startsWith(`${path}/`),
  ) ?? []
  await Promise.all(tabs.map(waitForTabUploads))
}

export function hasPendingTabUploads(tab: EditorTab) {
  return Boolean(pendingByTab.get(tab)?.size)
}
