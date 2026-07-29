# Marktree 测试

## 日常检查

```powershell
npm test
npm run build
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml
cargo check --manifest-path src-tauri/Cargo.toml --target x86_64-pc-windows-msvc
npm run check:android-rust
npm run android:build:debug
```

Windows 原生开发验证使用 `start-dev.bat` 或 `npm run desktop`，确认 Tauri 进程、原生窗口和前端内容都真实启动。`npm run check:android-rust` 只编译 Android `aarch64` Rust 链路，不生成 APK；脚本统一选择 `D:\Tools` 中的 Rust、Android NDK、Git Perl 支持模块和 NDK 构建工具，避免 vendored OpenSSL 在 Windows/MSYS 路径之间使用不一致的环境。Android 完整验收只生成本地 Universal Debug APK，不生成或发布 Release 包；验收时还要用 Android Build Tools 读取包名、版本和 SDK 边界，并确认 APK 中实际包含四个 ABI 的 `libmarktree_lib.so`。

## 真实 Git 集成测试

`src-tauri/src/git/tests.rs` 的测试不 mock libgit2。测试会在临时目录中创建普通仓库、Worktree 和裸远程仓库，并验证：

- 工作区状态和结构化 diff 来自真实文件修改；
- 同步事务只提交 Marktree 记录的文档，用户原有暂存内容和暂存删除在同步后仍留在索引中；
- 成功重试会清空完整的已记录变更清单，不遗留失效路径；
- 同步提交使用独立 index，进程在提交后退出并恢复时不会重复提交，也不会丢失用户原有暂存项；
- 文档经写入、提交和推送后，可被第二个真实克隆读取；
- 远端提交经 fetch 和 pull 后，第二个克隆的工作区文件真实更新；
- Worktree 由 libgit2 创建并能读取同一文档；
- 分支创建、切换、脏工作区拒绝切换和删除都作用于真实仓库；
- 两个克隆制造文本、二进制和删除冲突后，可用的祖先、本机与远端内容都进入恢复区；
- 冲突记录跨重启恢复，选择本机、远端或合并内容后可继续变基和推送；
- stash 在应用前退出、应用后退出和恢复中遇到新外部写入的窗口都经过真实仓库验证；中止操作也能跨重启完成；
- 第三个真实克隆最终读到冲突处理后的内容。

`src-tauri/src/documents/tests.rs`、`tests/editor-source.spec.ts` 和组件测试使用包含 Frontmatter、表格、任务、脚注、公式、Mermaid、中文、UTF-8 BOM、CRLF、混合换行和未知扩展的语料，验证未编辑内容逐字节不变、局部编辑不改写无关换行、外部写入不会被覆盖、配置文件外部修改不会被覆盖、独占创建不会替换现有文档、损坏的同名资源会被实际修复、脚注可见且不改写源文本。保存协调器测试验证同一标签页的连续修订不会乱序落盘。三方合并测试同时覆盖互不重叠编辑、重叠冲突和空祖先，冲突组件测试会执行真实候选选择并检查最终提交内容。

## 视觉检查

先启动本地预览：

```powershell
npm run dev -- --host 127.0.0.1
```

再运行：

```powershell
npm run test:visual
```

检查会使用系统 Edge 的无头模式打开独立的演示数据，验证亮色桌面、暗色紧凑桌面和暗色手机尺寸，确认前端真实启动、用户能看到编辑区、没有页面错误、横向溢出或移动端 Git 复杂界面泄漏，并保存截图到忽略提交的 `test-results/`。
