import { i18n } from '@/i18n'
import type { ErrorCode } from '@/types'

const nativeErrorMessageKeys: Record<ErrorCode, string> = {
  operationFailed: 'app.errorOperationFailed',
  gitFailed: 'app.errorGitFailed',
  fileFailed: 'app.errorFileFailed',
  invalidPath: 'app.errorInvalidPath',
  fileNotFound: 'app.errorFileNotFound',
  externalChange: 'app.errorExternalChange',
  managedContentChanged: 'app.errorManagedContentChanged',
  gitOperationPending: 'app.errorGitOperationPending',
  credentialFailed: 'app.errorCredentialFailed',
  networkFailed: 'app.errorNetworkFailed',
  watchFailed: 'app.errorWatchFailed',
  serializationFailed: 'app.errorSerializationFailed',
}

const errorsWithSelfContainedMessages = new Set<ErrorCode>([
  'externalChange',
  'managedContentChanged',
  'gitOperationPending',
])

export function readableError(reason: unknown): string {
  const native = nativeError(reason)
  if (native) {
    const summary = i18n.global.t(nativeErrorMessageKeys[native.code])
    return errorsWithSelfContainedMessages.has(native.code) || !native.message
      ? summary
      : `${summary} ${i18n.global.t('app.errorTechnicalDetail', { message: native.message })}`
  }
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

function nativeError(reason: unknown): { code: ErrorCode; message: string } | undefined {
  if (!reason || typeof reason !== 'object' || !('code' in reason)) return undefined
  const code = reason.code
  if (typeof code !== 'string' || !(code in nativeErrorMessageKeys)) return undefined
  const message = 'message' in reason && typeof reason.message === 'string' ? reason.message : ''
  return { code: code as ErrorCode, message }
}
