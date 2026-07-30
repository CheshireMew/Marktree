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
  GitBaselinePreview,
  GitStatusSnapshot,
  GithubDeviceCode,
  GithubDeviceToken,
  LocalStateSnapshot,
  MoveWorkspaceEntryRequest,
  PendingGitOperation,
  SaveDocumentRequest,
  SaveDocumentResult,
  SaveWorkspaceConfigRequest,
  SyncPlan,
  SyncResult,
  TextComparison,
  TrashEntry,
  WorkspaceConfigSnapshot,
  WorkspaceDescriptor,
  WorkspaceEntry,
  WorkspaceEntryMoveResult,
  WorktreeDescriptor,
  WorktreeSearchResult,
} from './native'

export interface OpenWorkspaceArgs extends Record<string, unknown> {
  path: string
}

export interface CreateWorkspaceArgs extends Record<string, unknown> {
  path: string
}

export interface CloneGitWorkspaceArgs extends Record<string, unknown> {
  remoteUrl: string
  path: string
  credentialId: string | null
}

export interface CloneMobileGitWorkspaceArgs extends Record<string, unknown> {
  remoteUrl: string
  workspaceName: string
  credentialId: string | null
}

export interface CreateMobileWorkspaceArgs extends Record<string, unknown> {
  workspaceName: string
}

export interface PreviewWorkspaceGitBaselineArgs extends Record<string, unknown> {
  root: string
}

export interface EnableWorkspaceGitArgs extends Record<string, unknown> {
  root: string
}

export interface ForgetWorkspaceArgs extends Record<string, unknown> {
  root: string
}

export interface RefreshWorkspaceArgs extends Record<string, unknown> {
  root: string
}

export interface WatchWorkspaceArgs extends Record<string, unknown> {
  root: string
}

export interface WorkspaceGitStatusArgs extends Record<string, unknown> {
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

export interface ListWorkspaceEntriesArgs extends Record<string, unknown> {
  root: string
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

export interface ReadWorkspaceConfigArgs extends Record<string, unknown> {
  root: string
}

export interface SaveWorkspaceConfigArgs extends Record<string, unknown> {
  request: SaveWorkspaceConfigRequest
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

export interface CreateWorkspaceFolderArgs extends Record<string, unknown> {
  root: string
  path: string
}

export interface MoveWorkspaceEntryArgs extends Record<string, unknown> {
  request: MoveWorkspaceEntryRequest
}

export interface TrashWorkspaceEntryArgs extends Record<string, unknown> {
  root: string
  path: string
}

export interface RestoreWorkspaceTrashArgs extends Record<string, unknown> {
  id: string
}

export interface OpenWorkspaceFileWithSystemArgs extends Record<string, unknown> {
  root: string
  path: string
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

export interface SyncWorkspaceChangesArgs extends Record<string, unknown> {
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

export interface SetWorkspaceGitCredentialArgs extends Record<string, unknown> {
  root: string
  credentialId: string
}

export interface PollGithubDeviceFlowArgs extends Record<string, unknown> {
  deviceCode: string
}

export const isTauri = () => '__TAURI_INTERNALS__' in window

export const nativeApi = {
  getLocalState: () => invoke<LocalStateSnapshot>('get_local_state'),
  openWorkspace: (args: OpenWorkspaceArgs) => invoke<WorkspaceDescriptor>('open_workspace', args),
  createWorkspace: (args: CreateWorkspaceArgs) => invoke<WorkspaceDescriptor>('create_workspace', args),
  cloneGitWorkspace: (args: CloneGitWorkspaceArgs) => invoke<WorkspaceDescriptor>('clone_git_workspace', args),
  cloneMobileGitWorkspace: (args: CloneMobileGitWorkspaceArgs) => invoke<WorkspaceDescriptor>('clone_mobile_git_workspace', args),
  createMobileWorkspace: (args: CreateMobileWorkspaceArgs) => invoke<WorkspaceDescriptor>('create_mobile_workspace', args),
  previewWorkspaceGitBaseline: (args: PreviewWorkspaceGitBaselineArgs) => invoke<GitBaselinePreview>('preview_workspace_git_baseline', args),
  enableWorkspaceGit: (args: EnableWorkspaceGitArgs) => invoke<WorkspaceDescriptor>('enable_workspace_git', args),
  forgetWorkspace: (args: ForgetWorkspaceArgs) => invoke<void>('forget_workspace', args),
  refreshWorkspace: (args: RefreshWorkspaceArgs) => invoke<WorkspaceDescriptor>('refresh_workspace', args),
  watchWorkspace: (args: WatchWorkspaceArgs) => invoke<void>('watch_workspace', args),
  workspaceGitStatus: (args: WorkspaceGitStatusArgs) => invoke<GitStatusSnapshot>('workspace_git_status', args),
  listBranches: (args: ListBranchesArgs) => invoke<BranchDescriptor[]>('list_branches', args),
  createBranch: (args: CreateBranchArgs) => invoke<GitStatusSnapshot>('create_branch', args),
  checkoutBranch: (args: CheckoutBranchArgs) => invoke<GitStatusSnapshot>('checkout_branch', args),
  deleteBranch: (args: DeleteBranchArgs) => invoke<BranchDescriptor[]>('delete_branch', args),
  createWorktree: (args: CreateWorktreeArgs) => invoke<WorktreeDescriptor>('create_worktree', args),
  listWorkspaceEntries: (args: ListWorkspaceEntriesArgs) => invoke<WorkspaceEntry[]>('list_workspace_entries', args),
  readDocument: (args: ReadDocumentArgs) => invoke<DocumentContent>('read_document', args),
  openDocument: (args: OpenDocumentArgs) => invoke<DocumentContent>('open_document', args),
  readAsset: (args: ReadAssetArgs) => invoke<AssetPreview>('read_asset', args),
  saveDocument: (args: SaveDocumentArgs) => invoke<SaveDocumentResult>('save_document', args),
  createDocument: (args: CreateDocumentArgs) => invoke<DocumentContent>('create_document', args),
  readWorkspaceConfig: (args: ReadWorkspaceConfigArgs) => invoke<WorkspaceConfigSnapshot>('read_workspace_config', args),
  saveWorkspaceConfig: (args: SaveWorkspaceConfigArgs) => invoke<WorkspaceConfigSnapshot>('save_workspace_config', args),
  writeAsset: (args: WriteAssetArgs) => invoke<AssetWriteResult>('write_asset', args),
  searchDocuments: (args: SearchDocumentsArgs) => invoke<string[]>('search_documents', args),
  createWorkspaceFolder: (args: CreateWorkspaceFolderArgs) => invoke<string>('create_workspace_folder', args),
  moveWorkspaceEntry: (args: MoveWorkspaceEntryArgs) => invoke<WorkspaceEntryMoveResult>('move_workspace_entry', args),
  trashWorkspaceEntry: (args: TrashWorkspaceEntryArgs) => invoke<TrashEntry | null>('trash_workspace_entry', args),
  listWorkspaceTrash: () => invoke<TrashEntry[]>('list_workspace_trash'),
  restoreWorkspaceTrash: (args: RestoreWorkspaceTrashArgs) => invoke<TrashEntry>('restore_workspace_trash', args),
  emptyWorkspaceTrash: () => invoke<void>('empty_workspace_trash'),
  openWorkspaceFileWithSystem: (args: OpenWorkspaceFileWithSystemArgs) => invoke<void>('open_workspace_file_with_system', args),
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
  syncWorkspaceChanges: (args: SyncWorkspaceChangesArgs) => invoke<SyncResult>('sync_workspace_changes', args),
  resolveConflict: (args: ResolveConflictArgs) => invoke<void>('resolve_conflict', args),
  resolveConflictWithContent: (args: ResolveConflictWithContentArgs) => invoke<void>('resolve_conflict_with_content', args),
  pendingConflicts: (args: PendingConflictsArgs) => invoke<ConflictRecord[]>('pending_conflicts', args),
  pendingGitOperation: (args: PendingGitOperationArgs) => invoke<PendingGitOperation | null>('pending_git_operation', args),
  resumeGitOperation: (args: ResumeGitOperationArgs) => invoke<SyncResult>('resume_git_operation', args),
  abortGitOperation: (args: AbortGitOperationArgs) => invoke<GitStatusSnapshot>('abort_git_operation', args),
  saveCredential: (args: SaveCredentialArgs) => invoke<void>('save_credential', args),
  setWorkspaceGitCredential: (args: SetWorkspaceGitCredentialArgs) => invoke<void>('set_workspace_git_credential', args),
  authConfiguration: () => invoke<AuthConfiguration>('auth_configuration'),
  beginGithubDeviceFlow: () => invoke<GithubDeviceCode>('begin_github_device_flow'),
  pollGithubDeviceFlow: (args: PollGithubDeviceFlowArgs) => invoke<GithubDeviceToken>('poll_github_device_flow', args),
}
