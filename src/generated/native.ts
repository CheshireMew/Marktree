// This file is generated from Rust serialization types. Do not edit it by hand.
// Run `npm run generate:bindings` after changing a native contract.

export interface RepositoryDescriptor {
  id: string
  name: string
  root: string
  commonDir: string
  remoteUrl: string | null
  worktrees: WorktreeDescriptor[]
  status: GitStatusSnapshot
}

export interface WorktreeDescriptor {
  name: string
  path: string
  branch: string | null
  isMain: boolean
  isLocked: boolean
  isDetached: boolean
  status: GitStatusSnapshot
}

export interface CreateWorktreeRequest {
  root: string
  name: string
  path: string
  branch: string
  startPoint: string | null
}

export interface BranchDescriptor {
  name: string
  isCurrent: boolean
  upstream: string | null
  ahead: number
  behind: number
  checkedOutPath: string | null
}

export interface GitStatusSnapshot {
  branch: string | null
  upstream: string | null
  ahead: number
  behind: number
  stagedCount: number
  changedCount: number
  untrackedCount: number
  conflictedCount: number
  files: GitFileStatus[]
}

export interface GitFileStatus {
  path: string
  indexStatus: string
  worktreeStatus: string
  staged: boolean
  conflicted: boolean
  untracked: boolean
}

export interface DocumentDescriptor {
  path: string
  name: string
  extension: string
  size: number
  modifiedMs: number
  readOnly: boolean
  kind: DocumentKind
  gitStatus: GitFileStatus | null
}

export type DocumentKind =
  | 'markdown'
  | 'text'
  | 'image'
  | 'other'

export type TextEncoding =
  | 'utf8'
  | 'utf8Bom'
  | 'unsupported'

export type LineEnding =
  | 'lf'
  | 'crlf'
  | 'cr'
  | 'mixed'
  | 'none'

export interface DocumentContent {
  path: string
  content: string
  modifiedMs: number
  sha256: string
  readOnly: boolean
  encoding: TextEncoding
  lineEnding: LineEnding
}

export interface RepositoryConfig {
  assetsDir: string
  ignoreRules: string[]
}

export interface RepositoryConfigSnapshot {
  config: RepositoryConfig
  sha256: string | null
  missing: boolean
}

export interface SaveRepositoryConfigRequest {
  root: string
  config: RepositoryConfig
  expectedSha256: string | null
  expectedMissing: boolean
}

export interface SaveDocumentRequest {
  root: string
  path: string
  content: string
  expectedSha256: string | null
  expectedMissing: boolean
  encoding: TextEncoding
}

export interface SaveDocumentResult {
  path: string
  modifiedMs: number
  sha256: string
  encoding: TextEncoding
  lineEnding: LineEnding
}

export interface AssetWriteResult {
  path: string
  markdownPath: string
  sha256: string
}

export interface AssetPreview {
  path: string
  mediaType: string
  base64Data: string
}

export type DiffMode =
  | 'worktreeToIndex'
  | 'indexToHead'
  | 'worktreeToHead'
  | 'localToUpstream'

export type ConflictChoice =
  | 'local'
  | 'remote'
  | 'merged'

export type ConflictKind =
  | 'text'
  | 'binary'
  | 'deleteModify'

export type GitOperationKind =
  | 'pull'
  | 'sync'

export type GitOperationPhase =
  | 'prepare'
  | 'commit'
  | 'fetch'
  | 'preserveWorkingTree'
  | 'rebase'
  | 'restoreWorkingTree'
  | 'push'
  | 'finalize'

export type SyncStage =
  | 'prepare'
  | 'credential'
  | 'stage'
  | 'commit'
  | 'fetch'
  | 'preserveWorkingTree'
  | 'rebase'
  | 'restoreWorkingTree'
  | 'push'
  | 'finalize'

export interface DiffResult {
  mode: DiffMode
  oldLabel: string
  newLabel: string
  insertions: number
  deletions: number
  files: DiffFile[]
}

export interface DiffFile {
  path: string
  oldPath: string | null
  status: string
  binary: boolean
  hunks: DiffHunk[]
}

export interface DiffHunk {
  header: string
  oldStart: number
  oldLines: number
  newStart: number
  newLines: number
  lines: DiffLine[]
}

export interface DiffLine {
  kind: string
  oldLine: number | null
  newLine: number | null
  content: string
}

export interface TextComparison {
  path: string
  leftLabel: string
  rightLabel: string
  left: string
  right: string
}

export interface WorktreeSearchResult {
  worktree: string
  root: string
  path: string
}

export interface SyncPlan {
  root: string
  branch: string | null
  changedPaths: string[]
  remoteUrl: string | null
  canPush: boolean
}

export type ManagedChangeKind =
  | 'document'
  | 'asset'
  | 'repositoryConfig'

export interface ManagedChange {
  path: string
  sha256: string
  generation: number
  kind: ManagedChangeKind
}

export interface PendingGitOperation {
  id: string
  root: string
  kind: GitOperationKind
  phase: GitOperationPhase
  startedAt: string
  managedChanges: ManagedChange[]
  changedPaths: string[]
  committed: boolean
  commitId: string | null
  pulled: boolean
  pushed: boolean
  originalHeadOid: string | null
  stashOid: string | null
  aborting: boolean
  stashApplyStarted: boolean
  stashApplied: boolean
}

export interface SyncResult {
  committed: boolean
  commitId: string | null
  pulled: boolean
  pushed: boolean
  changedPaths: string[]
  conflicts: ConflictRecord[]
  failureStage: SyncStage | null
  error: ErrorPayload | null
}

export interface ConflictRecord {
  path: string
  kind: ConflictKind
  ancestor: string | null
  local: string | null
  remote: string | null
  ancestorExists: boolean
  localExists: boolean
  remoteExists: boolean
  recoveryId: string
  choice: ConflictChoice | null
}

export interface CredentialInput {
  id: string
  username: string
  token: string
}

export interface CredentialRecord {
  username: string
  token: string
}

export interface GithubDeviceCode {
  deviceCode: string
  userCode: string
  verificationUri: string
  expiresIn: number
  interval: number
}

export interface GithubDeviceToken {
  accessToken: string | null
  tokenType: string | null
  scope: string | null
  pending: boolean
  error: string | null
}

export interface LocalStateSnapshot {
  repositories: string[]
  managedChanges: Record<string, ManagedChange[]>
  pendingGitOperations: Record<string, PendingGitOperation>
  recentFiles: string[]
  credentialRefs: Record<string, string>
}

export interface RepositoryChangedEvent {
  root: string
}

export interface RepositoryWatchErrorEvent {
  root: string
  message: string
}

export interface RepositoryForgottenEvent {
  repositoryId: string
  worktreeRoots: string[]
}

export type ErrorCode =
  | 'operationFailed'
  | 'gitFailed'
  | 'fileFailed'
  | 'invalidPath'
  | 'fileNotFound'
  | 'externalChange'
  | 'managedContentChanged'
  | 'gitOperationPending'
  | 'credentialFailed'
  | 'networkFailed'
  | 'watchFailed'
  | 'serializationFailed'

export interface ErrorPayload {
  code: ErrorCode
  message: string
}

export interface AuthConfiguration {
  githubClientId: string
  githubEnabled: boolean
}

export type NativeError = ErrorPayload
