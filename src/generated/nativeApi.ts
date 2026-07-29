// This file is generated from the Rust command registry and signatures.
// Do not edit it by hand. Run `npm run generate:bindings` instead.

import { invoke } from '@tauri-apps/api/core'

import type {
  AssetPreview,
  AssetWriteResult,
  AuthConfiguration,
  BranchDescriptor,
  ConflictChoice,
  ConflictRecord,
  CreateWorktreeRequest,
  CredentialInput,
  DiffMode,
  DiffResult,
  DocumentContent,
  DocumentDescriptor,
  GitFileStatus,
  GitStatusSnapshot,
  GithubDeviceCode,
  GithubDeviceToken,
  LocalStateSnapshot,
  PendingGitOperation,
  RepositoryConfigSnapshot,
  RepositoryDescriptor,
  SaveDocumentRequest,
  SaveDocumentResult,
  SaveRepositoryConfigRequest,
  SyncPlan,
  SyncResult,
  TextComparison,
  WorktreeDescriptor,
  WorktreeSearchResult,
} from './native'

export interface OpenRepositoryArgs extends Record<string, unknown> {
  path: string
}

export interface InitializeRepositoryArgs extends Record<string, unknown> {
  path: string
}

export interface CloneRepositoryArgs extends Record<string, unknown> {
  remoteUrl: string
  path: string
  credentialId: string | null
}

export interface CloneMobileRepositoryArgs extends Record<string, unknown> {
  remoteUrl: string
  repositoryName: string
  credentialId: string | null
}

export interface InitializeMobileRepositoryArgs extends Record<string, unknown> {
  repositoryName: string
}

export interface ForgetRepositoryArgs extends Record<string, unknown> {
  root: string
}

export interface RefreshRepositoryArgs extends Record<string, unknown> {
  root: string
}

export interface RepositoryStatusArgs extends Record<string, unknown> {
  root: string
}

export interface ListBranchesArgs extends Record<string, unknown> {
  root: string
}

export interface CreateBranchArgs extends Record<string, unknown> {
  root: string
  name: string
  startPoint: string | null
  checkout: boolean
}

export interface CheckoutBranchArgs extends Record<string, unknown> {
  root: string
  name: string
}

export interface DeleteBranchArgs extends Record<string, unknown> {
  root: string
  name: string
}

export interface CreateWorktreeArgs extends Record<string, unknown> {
  request: CreateWorktreeRequest
}

export interface WatchRepositoryArgs extends Record<string, unknown> {
  root: string
}

export interface ListDocumentsArgs extends Record<string, unknown> {
  root: string
  statuses: GitFileStatus[]
}

export interface ReadDocumentArgs extends Record<string, unknown> {
  root: string
  path: string
}

export interface OpenDocumentArgs extends Record<string, unknown> {
  root: string
  path: string
}

export interface ReadAssetArgs extends Record<string, unknown> {
  root: string
  path: string
}

export interface SaveDocumentArgs extends Record<string, unknown> {
  request: SaveDocumentRequest
}

export interface CreateDocumentArgs extends Record<string, unknown> {
  root: string
  path: string
}

export interface ReadRepositoryConfigArgs extends Record<string, unknown> {
  root: string
}

export interface SaveRepositoryConfigArgs extends Record<string, unknown> {
  request: SaveRepositoryConfigRequest
}

export interface WriteAssetArgs extends Record<string, unknown> {
  root: string
  documentPath: string
  fileName: string
  base64Data: string
  assetsDir: string | null
}

export interface SearchDocumentsArgs extends Record<string, unknown> {
  root: string
  query: string
  limit: number
}

export interface SearchWorktreesArgs extends Record<string, unknown> {
  root: string
  query: string
  limit: number
}

export interface StagePathsArgs extends Record<string, unknown> {
  root: string
  paths: string[]
}

export interface StageAllArgs extends Record<string, unknown> {
  root: string
}

export interface UnstagePathsArgs extends Record<string, unknown> {
  root: string
  paths: string[]
}

export interface CommitArgs extends Record<string, unknown> {
  root: string
  message: string
}

export interface FetchArgs extends Record<string, unknown> {
  root: string
}

export interface PullRebaseArgs extends Record<string, unknown> {
  root: string
}

export interface PushArgs extends Record<string, unknown> {
  root: string
}

export interface GitDiffArgs extends Record<string, unknown> {
  root: string
  mode: DiffMode
}

export interface CompareWorktreesArgs extends Record<string, unknown> {
  leftRoot: string
  rightRoot: string
  path: string
}

export interface SyncPlanArgs extends Record<string, unknown> {
  root: string
}

export interface SyncMarktreeChangesArgs extends Record<string, unknown> {
  root: string
}

export interface ResolveConflictArgs extends Record<string, unknown> {
  root: string
  path: string
  recoveryId: string
  choice: ConflictChoice
}

export interface ResolveConflictWithContentArgs extends Record<string, unknown> {
  root: string
  path: string
  recoveryId: string
  content: string
}

export interface PendingConflictsArgs extends Record<string, unknown> {
  root: string
}

export interface PendingGitOperationArgs extends Record<string, unknown> {
  root: string
}

export interface ResumeGitOperationArgs extends Record<string, unknown> {
  root: string
}

export interface AbortGitOperationArgs extends Record<string, unknown> {
  root: string
}

export interface SaveCredentialArgs extends Record<string, unknown> {
  input: CredentialInput
}

export interface SetRepositoryCredentialArgs extends Record<string, unknown> {
  root: string
  credentialId: string
}

export interface PollGithubDeviceFlowArgs extends Record<string, unknown> {
  deviceCode: string
}

export const isTauri = () => '__TAURI_INTERNALS__' in window

export const nativeApi = {
  getLocalState: () => invoke<LocalStateSnapshot>('get_local_state'),
  openRepository: (args: OpenRepositoryArgs) => invoke<RepositoryDescriptor>('open_repository', args),
  initializeRepository: (args: InitializeRepositoryArgs) => invoke<RepositoryDescriptor>('initialize_repository', args),
  cloneRepository: (args: CloneRepositoryArgs) => invoke<RepositoryDescriptor>('clone_repository', args),
  cloneMobileRepository: (args: CloneMobileRepositoryArgs) => invoke<RepositoryDescriptor>('clone_mobile_repository', args),
  initializeMobileRepository: (args: InitializeMobileRepositoryArgs) => invoke<RepositoryDescriptor>('initialize_mobile_repository', args),
  forgetRepository: (args: ForgetRepositoryArgs) => invoke<void>('forget_repository', args),
  refreshRepository: (args: RefreshRepositoryArgs) => invoke<RepositoryDescriptor>('refresh_repository', args),
  repositoryStatus: (args: RepositoryStatusArgs) => invoke<GitStatusSnapshot>('repository_status', args),
  listBranches: (args: ListBranchesArgs) => invoke<BranchDescriptor[]>('list_branches', args),
  createBranch: (args: CreateBranchArgs) => invoke<GitStatusSnapshot>('create_branch', args),
  checkoutBranch: (args: CheckoutBranchArgs) => invoke<GitStatusSnapshot>('checkout_branch', args),
  deleteBranch: (args: DeleteBranchArgs) => invoke<BranchDescriptor[]>('delete_branch', args),
  createWorktree: (args: CreateWorktreeArgs) => invoke<WorktreeDescriptor>('create_worktree', args),
  watchRepository: (args: WatchRepositoryArgs) => invoke<void>('watch_repository', args),
  listDocuments: (args: ListDocumentsArgs) => invoke<DocumentDescriptor[]>('list_documents', args),
  readDocument: (args: ReadDocumentArgs) => invoke<DocumentContent>('read_document', args),
  openDocument: (args: OpenDocumentArgs) => invoke<DocumentContent>('open_document', args),
  readAsset: (args: ReadAssetArgs) => invoke<AssetPreview>('read_asset', args),
  saveDocument: (args: SaveDocumentArgs) => invoke<SaveDocumentResult>('save_document', args),
  createDocument: (args: CreateDocumentArgs) => invoke<DocumentContent>('create_document', args),
  readRepositoryConfig: (args: ReadRepositoryConfigArgs) => invoke<RepositoryConfigSnapshot>('read_repository_config', args),
  saveRepositoryConfig: (args: SaveRepositoryConfigArgs) => invoke<RepositoryConfigSnapshot>('save_repository_config', args),
  writeAsset: (args: WriteAssetArgs) => invoke<AssetWriteResult>('write_asset', args),
  searchDocuments: (args: SearchDocumentsArgs) => invoke<string[]>('search_documents', args),
  searchWorktrees: (args: SearchWorktreesArgs) => invoke<WorktreeSearchResult[]>('search_worktrees', args),
  stagePaths: (args: StagePathsArgs) => invoke<GitStatusSnapshot>('stage_paths', args),
  stageAll: (args: StageAllArgs) => invoke<GitStatusSnapshot>('stage_all', args),
  unstagePaths: (args: UnstagePathsArgs) => invoke<GitStatusSnapshot>('unstage_paths', args),
  commit: (args: CommitArgs) => invoke<string>('commit', args),
  fetch: (args: FetchArgs) => invoke<GitStatusSnapshot>('fetch', args),
  pullRebase: (args: PullRebaseArgs) => invoke<SyncResult>('pull_rebase', args),
  push: (args: PushArgs) => invoke<GitStatusSnapshot>('push', args),
  gitDiff: (args: GitDiffArgs) => invoke<DiffResult>('git_diff', args),
  compareWorktrees: (args: CompareWorktreesArgs) => invoke<TextComparison>('compare_worktrees', args),
  syncPlan: (args: SyncPlanArgs) => invoke<SyncPlan>('sync_plan', args),
  syncMarktreeChanges: (args: SyncMarktreeChangesArgs) => invoke<SyncResult>('sync_marktree_changes', args),
  resolveConflict: (args: ResolveConflictArgs) => invoke<void>('resolve_conflict', args),
  resolveConflictWithContent: (args: ResolveConflictWithContentArgs) => invoke<void>('resolve_conflict_with_content', args),
  pendingConflicts: (args: PendingConflictsArgs) => invoke<ConflictRecord[]>('pending_conflicts', args),
  pendingGitOperation: (args: PendingGitOperationArgs) => invoke<PendingGitOperation | null>('pending_git_operation', args),
  resumeGitOperation: (args: ResumeGitOperationArgs) => invoke<SyncResult>('resume_git_operation', args),
  abortGitOperation: (args: AbortGitOperationArgs) => invoke<GitStatusSnapshot>('abort_git_operation', args),
  saveCredential: (args: SaveCredentialArgs) => invoke<void>('save_credential', args),
  setRepositoryCredential: (args: SetRepositoryCredentialArgs) => invoke<void>('set_repository_credential', args),
  authConfiguration: () => invoke<AuthConfiguration>('auth_configuration'),
  beginGithubDeviceFlow: () => invoke<GithubDeviceCode>('begin_github_device_flow'),
  pollGithubDeviceFlow: (args: PollGithubDeviceFlowArgs) => invoke<GithubDeviceToken>('poll_github_device_flow', args),
}
