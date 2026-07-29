export function readableError(reason: unknown): string {
  if (reason instanceof Error) return reason.message
  if (typeof reason === 'string') return reason
  if (
    reason &&
    typeof reason === 'object' &&
    'message' in reason &&
    typeof reason.message === 'string'
  ) {
    return reason.message
  }
  try {
    return JSON.stringify(reason)
  } catch {
    return 'Unknown error'
  }
}
