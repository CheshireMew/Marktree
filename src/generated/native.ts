// This file is generated from the reachable Rust IPC serialization types. Do not edit it by hand.
// Run `npm run generate:bindings` after changing a native contract.

/** A Rust 64-bit or pointer-sized integer carried as a JSON number; IPC values must remain within JavaScript's safe-integer range. */
export type SafeInteger = number

export interface WorkspaceDescriptor {
  id: string
  name: string
  root: string
  git: GitCapability | null
}

export interface WorkspaceEntry {
  path: string
  name: string
  entryType: WorkspaceEntryType
  fileKind: DocumentKind | null
  size: SafeInteger
  modifiedMs: SafeInteger
  readOnly: boolean
  gitStatus: GitFileStatus | null
}

export interface WorkspaceViewSnapshot {
  entries: WorkspaceEntry[]
  status: GitStatusSnapshot | null
  branches: BranchDescriptor[]
  pendingOperation: PendingGitOperationSummary | null
  conflicts: ConflictRecord[]
}

export interface WorkspaceEntriesPatch {
  entries: WorkspaceEntry[]
  removedPaths: string[]
  status: GitStatusSnapshot | null
  fullReloadRequired: boolean
}

export interface WorkspaceRefreshSnapshot {
  workspace: WorkspaceDescriptor
  view: WorkspaceViewSnapshot
}

export interface GitBaselinePreview {
  fileCount: SafeInteger
  totalBytes: SafeInteger
  ignoredCount: SafeInteger
  ignoreRules: string[]
}

export interface MoveWorkspaceEntryRequest {
  root: string
  sourcePath: string
  destinationPath: string
}

export interface DuplicateWorkspaceEntryRequest {
  root: string
  sourcePath: string
  destinationPath: string
}

export interface WorkspaceEntryMoveResult {
  oldPath: string
  newPath: string
  movedFiles: WorkspacePathMove[]
}

export interface WorkspaceEntryDuplicateResult {
  sourcePath: string
  newPath: string
  copiedFiles: WorkspacePathMove[]
}

export interface WorkspacePathMove {
  oldPath: string
  newPath: string
}

export interface TrashEntry {
  id: string
  workspaceRoot: string
  originalPath: string
  name: string
  deletedAt: string
}

export interface CredentialInput {
  id: string
  username: string
  token: string
}

export interface GithubDeviceCode {
  deviceCode: string
  userCode: string
  verificationUri: string
  expiresIn: SafeInteger
  interval: SafeInteger
}

export interface GithubDeviceToken {
  accessToken: string | null
  tokenType: string | null
  scope: string | null
  pending: boolean
  error: string | null
}

export type WorkspaceEntryType =
  | 'directory'
  | 'file'

export type DocumentKind =
  | 'markdown'
  | 'text'
  | 'image'
  | 'pdf'
  | 'audio'
  | 'video'
  | 'other'

export type DocumentSearchMatchType =
  | 'path'
  | 'content'

export interface DocumentSearchResult {
  path: string
  line: SafeInteger | null
  column: SafeInteger | null
  snippet: string
  matchType: DocumentSearchMatchType
  fileKind: DocumentKind
  modifiedMs: SafeInteger
}

export interface SearchStatistics {
  scannedFiles: SafeInteger
  scannedBytes: SafeInteger
  skippedLargeFiles: SafeInteger
  truncated: boolean
}

export interface DocumentSearchResponse {
  results: DocumentSearchResult[]
  statistics: SearchStatistics
}

export interface DocumentSearchRequest {
  root: string
  query: string
  limit: SafeInteger
  pathPrefix: string | null
  fileKinds: DocumentKind[]
  modifiedAfterMs: SafeInteger | null
}

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
  modifiedMs: SafeInteger
  sha256: string
  readOnly: boolean
  encoding: TextEncoding
  lineEnding: LineEnding
}

export interface WorkspaceConfig {
  assetsDir: string
  ignoreRules: string[]
}

export interface WorkspaceConfigSnapshot {
  config: WorkspaceConfig
  sha256: string | null
  missing: boolean
}

export interface SaveWorkspaceConfigRequest {
  root: string
  config: WorkspaceConfig
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
  modifiedMs: SafeInteger
  sha256: string
  encoding: TextEncoding
  lineEnding: LineEnding
}

export interface AssetWriteResult {
  path: string
  markdownPath: string
  sha256: string
}

export interface BeginAssetUploadRequest {
  root: string
  documentPath: string
  fileName: string
  assetsDir: string | null
  totalBytes: SafeInteger
}

export interface AssetUploadTicket {
  id: string
  maxChunkBytes: SafeInteger
}

export interface AssetUploadChunkRequest {
  uploadId: string
  offset: SafeInteger
  base64Data: string
}

export interface WorkspaceFilePreview {
  path: string
  kind: DocumentKind
  mediaType: string
  resourcePath: string
}

export interface GitCapability {
  commonDir: string
  remoteUrl: string | null
  worktrees: WorktreeDescriptor[]
  status: GitStatusSnapshot | null
}

export interface WorktreeDescriptor {
  name: string
  path: string
  branch: string | null
  isMain: boolean
  isLocked: boolean
  isDetached: boolean
  status: GitStatusSnapshot | null
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
  ahead: SafeInteger
  behind: SafeInteger
  checkedOutPath: string | null
}

export interface GitStatusSnapshot {
  branch: string | null
  upstream: string | null
  ahead: SafeInteger
  behind: SafeInteger
  stagedCount: SafeInteger
  changedCount: SafeInteger
  untrackedCount: SafeInteger
  conflictedCount: SafeInteger
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
  insertions: SafeInteger
  deletions: SafeInteger
  truncated: boolean
  omittedLines: SafeInteger
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
  line: SafeInteger | null
  column: SafeInteger | null
  snippet: string
  matchType: DocumentSearchMatchType
  fileKind: DocumentKind
  modifiedMs: SafeInteger
}

export interface WorktreeSearchResponse {
  results: WorktreeSearchResult[]
  statistics: SearchStatistics
}

export interface WorktreeSearchRequest {
  root: string
  query: string
  limit: SafeInteger
  pathPrefix: string | null
  fileKinds: DocumentKind[]
  modifiedAfterMs: SafeInteger | null
}

export interface SyncPlan {
  root: string
  branch: string | null
  changedPaths: string[]
  remoteUrl: string | null
  canPush: boolean
}

export interface PendingGitOperationSummary {
  id: string
  root: string
  kind: GitOperationKind
  phase: GitOperationPhase
  startedAt: string
  aborting: boolean
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

export interface WorkspaceArchiveExportResult {
  fileCount: SafeInteger
  totalBytes: SafeInteger
}

export interface AndroidShareImportResult {
  workspace: WorkspaceDescriptor
  openPath: string | null
  insertMarkdown: string | null
  archiveImported: boolean
}

export type AndroidShareKind =
  | 'text'
  | 'markdown'
  | 'image'
  | 'attachment'
  | 'archive'

export interface PendingAndroidShare {
  text: string | null
  subject: string | null
  filePath: string | null
  fileName: string | null
  mediaType: string | null
  kind: AndroidShareKind
}

export interface ImportAndroidShareRequest {
  share: PendingAndroidShare
  root: string | null
  targetDirectory: string
  documentPath: string | null
}

export type OperationLogCategory =
  | 'workspace'
  | 'git'
  | 'recovery'

export type OperationLogOutcome =
  | 'started'
  | 'progress'
  | 'succeeded'
  | 'cancelled'
  | 'failed'

export interface OperationLogEntry {
  timestamp: string
  category: OperationLogCategory
  action: string
  phase: string
  outcome: OperationLogOutcome
  root: string | null
  operationId: string | null
  errorCode: ErrorCode | null
}

export interface StartupState {
  workspaces: string[]
  recentFiles: string[]
  recentFileLimit: SafeInteger
}

export interface WorkspaceChangedEvent {
  root: string
  paths: string[]
}

export interface WorkspaceWatchErrorEvent {
  root: string
  message: string
}

export interface WorkspaceForgottenEvent {
  workspaceId: string
  roots: string[]
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
