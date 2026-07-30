# Marktree repository guidance

Marktree is a source-faithful Markdown workspace for Windows and Android.

## Product invariants

- Files inside the user-selected workspace folder are the only source of truth for document content. A workspace does not require Git.
- Git is an optional capability. It owns history and cross-device synchronization only when the workspace root itself is a Git repository; parent-directory discovery is forbidden.
- Application state may store recent workspace and file paths, UI state, credential references, Git change manifests, and recovery copies. It must not mirror active Markdown or text content.
- Vue owns presentation and interaction. Rust/Tauri owns workspace-bound filesystem access, optional Git, credentials, and exact-root capability detection.
- Android creates ordinary workspaces inside application-private storage and may additionally clone or initialize Git. Desktop additionally exposes complete file management and advanced Git tools.
- Do not add a web runtime, Python service, content database, Bemo compatibility layer, or proprietary document format.

## Verification

- Plain-workspace tests use real directories without `.git` and prove no Git change manifest is generated.
- Git tests use real repositories and bare remotes, and nested repositories are tested as ordinary folders unless opened directly.
- Source-fidelity tests prove that opening and saving an unchanged Markdown document is byte-for-byte stable.
- Core-chain tests must verify both plain file operations and the optional Git path where a saved or moved file becomes an exact Git change, is committed and pushed, and can be read from a second clone.
- Run frontend tests, Rust tests, the production frontend build, the Windows native check, and the Android debug check for release-facing changes.
