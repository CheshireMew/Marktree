# Marktree repository guidance

Marktree is a source-faithful Markdown workspace for Windows and Android.

## Product invariants

- Markdown files in Git worktrees are the only source of truth for document content.
- Git repositories are the only source of truth for history and cross-device synchronization.
- Application state may store recent paths, UI state, credential references, changed-path manifests, and recovery copies. It must not mirror active Markdown content.
- Vue owns presentation and interaction. Rust/Tauri owns filesystem, Git, credentials, and repository discovery.
- Android exposes a simple synchronize flow. Desktop additionally exposes worktrees and the daily Git workflow.
- Do not add a web runtime, Python service, content database, Bemo compatibility layer, or proprietary document format.

## Verification

- Git tests use real repositories and bare remotes.
- Source-fidelity tests prove that opening and saving an unchanged Markdown document is byte-for-byte stable.
- Core-chain tests must verify that a saved file becomes a Git change, is committed and pushed, and can be read from a second clone.
- Run frontend tests, Rust tests, the production frontend build, the Windows native check, and the Android debug check for release-facing changes.
