// This file is generated from the Rust command registry and signatures.
// Do not edit it by hand. Run `npm run generate:bindings` instead.

import { invoke } from '@tauri-apps/api/core'

import type {
  AndroidShareImportResult,
  AssetUploadChunkRequest,
  AssetUploadTicket,
  AssetWriteResult,
  AuthConfiguration,
  BeginAssetUploadRequest,
  BranchDescriptor,
  ConflictChoice,
  CreateWorktreeRequest,
  CredentialInput,
  DiffMode,
  DiffResult,
  DocumentContent,
  DocumentSearchRequest,
  DocumentSearchResponse,
  DuplicateWorkspaceEntryRequest,
  GitBaselinePreview,
  GitStatusSnapshot,
  GithubDeviceCode,
  GithubDeviceToken,
  ImportAndroidShareRequest,
  MoveWorkspaceEntryRequest,
  OperationLogEntry,
  PendingAndroidShare,
  PendingGitOperationSummary,
  SafeInteger,
  SaveDocumentRequest,
  SaveDocumentResult,
  SaveWorkspaceConfigRequest,
  StartupState,
  SyncPlan,
  SyncResult,
  TextComparison,
  TrashEntry,
  WorkspaceArchiveExportResult,
  WorkspaceConfigSnapshot,
  WorkspaceDescriptor,
  WorkspaceEntriesPatch,
  WorkspaceEntryDuplicateResult,
  WorkspaceEntryMoveResult,
  WorkspaceFilePreview,
  WorkspaceRefreshSnapshot,
  WorkspaceViewSnapshot,
  WorktreeDescriptor,
  WorktreeSearchRequest,
  WorktreeSearchResponse,
} from './native'

export interface ReadOperationLogArgs extends Record<string, unknown> {
  limit: SafeInteger
}

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

export interface RefreshWorkspaceViewArgs extends Record<string, unknown> {
  workspaceRoot: string
  contentRoot: string
}

export interface WatchWorkspaceArgs extends Record<string, unknown> {
  root: string
}

export interface UnwatchWorkspaceArgs extends Record<string, unknown> {
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

export interface WorkspaceViewArgs extends Record<string, unknown> {
  root: string
}

export interface ListWorkspaceDirectoriesArgs extends Record<string, unknown> {
  root: string
}

export interface WorkspaceEntriesPatchArgs extends Record<string, unknown> {
  root: string
  paths: string[]
}

export interface CancelSearchesArgs extends Record<string, unknown> {
  roots: string[]
}

export interface ReadDocumentArgs extends Record<string, unknown> {
  root: string
  path: string
}

export interface OpenDocumentArgs extends Record<string, unknown> {
  root: string
  path: string
}

export interface ReadWorkspacePreviewArgs extends Record<string, unknown> {
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

export interface BeginAssetUploadArgs extends Record<string, unknown> {
  request: BeginAssetUploadRequest
}

export interface AppendAssetUploadArgs extends Record<string, unknown> {
  request: AssetUploadChunkRequest
}

export interface FinishAssetUploadArgs extends Record<string, unknown> {
  uploadId: string
}

export interface AbortAssetUploadArgs extends Record<string, unknown> {
  uploadId: string
}

export interface SearchDocumentsArgs extends Record<string, unknown> {
  request: DocumentSearchRequest
}

export interface CreateWorkspaceFolderArgs extends Record<string, unknown> {
  root: string
  path: string
}

export interface MoveWorkspaceEntryArgs extends Record<string, unknown> {
  request: MoveWorkspaceEntryRequest
}

export interface DuplicateWorkspaceEntryArgs extends Record<string, unknown> {
  request: DuplicateWorkspaceEntryRequest
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

export interface ImportAndroidShareArgs extends Record<string, unknown> {
  request: ImportAndroidShareRequest
}

export interface ExportAndroidWorkspaceArchiveArgs extends Record<string, unknown> {
  root: string
}

export interface SearchWorktreesArgs extends Record<string, unknown> {
  request: WorktreeSearchRequest
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
  getStartupState: () => invoke<StartupState>('get_startup_state'),
  readOperationLog: (args: ReadOperationLogArgs) => invoke<OperationLogEntry[]>('read_operation_log', args),
  openWorkspace: (args: OpenWorkspaceArgs) => invoke<WorkspaceDescriptor>('open_workspace', args),
  createWorkspace: (args: CreateWorkspaceArgs) => invoke<WorkspaceDescriptor>('create_workspace', args),
  cloneGitWorkspace: (args: CloneGitWorkspaceArgs) => invoke<WorkspaceDescriptor>('clone_git_workspace', args),
  cloneMobileGitWorkspace: (args: CloneMobileGitWorkspaceArgs) => invoke<WorkspaceDescriptor>('clone_mobile_git_workspace', args),
  createMobileWorkspace: (args: CreateMobileWorkspaceArgs) => invoke<WorkspaceDescriptor>('create_mobile_workspace', args),
  previewWorkspaceGitBaseline: (args: PreviewWorkspaceGitBaselineArgs) => invoke<GitBaselinePreview>('preview_workspace_git_baseline', args),
  enableWorkspaceGit: (args: EnableWorkspaceGitArgs) => invoke<WorkspaceDescriptor>('enable_workspace_git', args),
  forgetWorkspace: (args: ForgetWorkspaceArgs) => invoke<void>('forget_workspace', args),
  refreshWorkspaceView: (args: RefreshWorkspaceViewArgs) => invoke<WorkspaceRefreshSnapshot>('refresh_workspace_view', args),
  watchWorkspace: (args: WatchWorkspaceArgs) => invoke<void>('watch_workspace', args),
  unwatchWorkspace: (args: UnwatchWorkspaceArgs) => invoke<void>('unwatch_workspace', args),
  createBranch: (args: CreateBranchArgs) => invoke<WorkspaceViewSnapshot>('create_branch', args),
  checkoutBranch: (args: CheckoutBranchArgs) => invoke<WorkspaceViewSnapshot>('checkout_branch', args),
  deleteBranch: (args: DeleteBranchArgs) => invoke<BranchDescriptor[]>('delete_branch', args),
  createWorktree: (args: CreateWorktreeArgs) => invoke<WorktreeDescriptor>('create_worktree', args),
  workspaceView: (args: WorkspaceViewArgs) => invoke<WorkspaceViewSnapshot>('workspace_view', args),
  listWorkspaceDirectories: (args: ListWorkspaceDirectoriesArgs) => invoke<string[]>('list_workspace_directories', args),
  workspaceEntriesPatch: (args: WorkspaceEntriesPatchArgs) => invoke<WorkspaceEntriesPatch>('workspace_entries_patch', args),
  cancelSearches: (args: CancelSearchesArgs) => invoke<void>('cancel_searches', args),
  readDocument: (args: ReadDocumentArgs) => invoke<DocumentContent>('read_document', args),
  openDocument: (args: OpenDocumentArgs) => invoke<DocumentContent>('open_document', args),
  readWorkspacePreview: (args: ReadWorkspacePreviewArgs) => invoke<WorkspaceFilePreview>('read_workspace_preview', args),
  saveDocument: (args: SaveDocumentArgs) => invoke<SaveDocumentResult>('save_document', args),
  createDocument: (args: CreateDocumentArgs) => invoke<DocumentContent>('create_document', args),
  readWorkspaceConfig: (args: ReadWorkspaceConfigArgs) => invoke<WorkspaceConfigSnapshot>('read_workspace_config', args),
  saveWorkspaceConfig: (args: SaveWorkspaceConfigArgs) => invoke<WorkspaceConfigSnapshot>('save_workspace_config', args),
  beginAssetUpload: (args: BeginAssetUploadArgs) => invoke<AssetUploadTicket>('begin_asset_upload', args),
  appendAssetUpload: (args: AppendAssetUploadArgs) => invoke<void>('append_asset_upload', args),
  finishAssetUpload: (args: FinishAssetUploadArgs) => invoke<AssetWriteResult>('finish_asset_upload', args),
  abortAssetUpload: (args: AbortAssetUploadArgs) => invoke<void>('abort_asset_upload', args),
  searchDocuments: (args: SearchDocumentsArgs) => invoke<DocumentSearchResponse>('search_documents', args),
  createWorkspaceFolder: (args: CreateWorkspaceFolderArgs) => invoke<string>('create_workspace_folder', args),
  moveWorkspaceEntry: (args: MoveWorkspaceEntryArgs) => invoke<WorkspaceEntryMoveResult>('move_workspace_entry', args),
  duplicateWorkspaceEntry: (args: DuplicateWorkspaceEntryArgs) => invoke<WorkspaceEntryDuplicateResult>('duplicate_workspace_entry', args),
  trashWorkspaceEntry: (args: TrashWorkspaceEntryArgs) => invoke<TrashEntry | null>('trash_workspace_entry', args),
  listWorkspaceTrash: () => invoke<TrashEntry[]>('list_workspace_trash'),
  restoreWorkspaceTrash: (args: RestoreWorkspaceTrashArgs) => invoke<TrashEntry>('restore_workspace_trash', args),
  emptyWorkspaceTrash: () => invoke<void>('empty_workspace_trash'),
  openWorkspaceFileWithSystem: (args: OpenWorkspaceFileWithSystemArgs) => invoke<void>('open_workspace_file_with_system', args),
  takePendingAndroidShare: () => invoke<PendingAndroidShare | null>('take_pending_android_share'),
  importAndroidShare: (args: ImportAndroidShareArgs) => invoke<AndroidShareImportResult>('import_android_share', args),
  exportAndroidWorkspaceArchive: (args: ExportAndroidWorkspaceArchiveArgs) => invoke<WorkspaceArchiveExportResult>('export_android_workspace_archive', args),
  searchWorktrees: (args: SearchWorktreesArgs) => invoke<WorktreeSearchResponse>('search_worktrees', args),
  stagePaths: (args: StagePathsArgs) => invoke<WorkspaceViewSnapshot>('stage_paths', args),
  stageAll: (args: StageAllArgs) => invoke<WorkspaceViewSnapshot>('stage_all', args),
  unstagePaths: (args: UnstagePathsArgs) => invoke<WorkspaceViewSnapshot>('unstage_paths', args),
  commit: (args: CommitArgs) => invoke<WorkspaceViewSnapshot>('commit', args),
  fetch: (args: FetchArgs) => invoke<void>('fetch', args),
  pullRebase: (args: PullRebaseArgs) => invoke<SyncResult>('pull_rebase', args),
  push: (args: PushArgs) => invoke<void>('push', args),
  gitDiff: (args: GitDiffArgs) => invoke<DiffResult>('git_diff', args),
  compareWorktrees: (args: CompareWorktreesArgs) => invoke<TextComparison>('compare_worktrees', args),
  syncPlan: (args: SyncPlanArgs) => invoke<SyncPlan>('sync_plan', args),
  syncWorkspaceChanges: (args: SyncWorkspaceChangesArgs) => invoke<SyncResult>('sync_workspace_changes', args),
  resolveConflict: (args: ResolveConflictArgs) => invoke<void>('resolve_conflict', args),
  resolveConflictWithContent: (args: ResolveConflictWithContentArgs) => invoke<void>('resolve_conflict_with_content', args),
  pendingGitOperation: (args: PendingGitOperationArgs) => invoke<PendingGitOperationSummary | null>('pending_git_operation', args),
  resumeGitOperation: (args: ResumeGitOperationArgs) => invoke<SyncResult>('resume_git_operation', args),
  abortGitOperation: (args: AbortGitOperationArgs) => invoke<GitStatusSnapshot>('abort_git_operation', args),
  saveCredential: (args: SaveCredentialArgs) => invoke<void>('save_credential', args),
  setWorkspaceGitCredential: (args: SetWorkspaceGitCredentialArgs) => invoke<void>('set_workspace_git_credential', args),
  authConfiguration: () => invoke<AuthConfiguration>('auth_configuration'),
  beginGithubDeviceFlow: () => invoke<GithubDeviceCode>('begin_github_device_flow'),
  pollGithubDeviceFlow: (args: PollGithubDeviceFlowArgs) => invoke<GithubDeviceToken>('poll_github_device_flow', args),
}
