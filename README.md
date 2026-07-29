# Marktree

Marktree 是一个以 Markdown 文件和 Git 仓库为核心的 Windows / Android 写作工具。它直接编辑仓库里的源文件，不建立笔记数据库，也不要求用户把内容导入某种专有格式。

当前仓库是源码开发阶段，不提供安装包或应用商店版本。

## 已实现

- Vue 3、TypeScript 与 Tauri 2 共用 Windows / Android 界面和领域模型。
- Rust `git2/libgit2` 核心，无需系统安装 Git。
- Windows 多仓库、多 Worktree、独立窗口与日常 Git 操作。
- Android 应用私有仓库、简化界面与一键同步。
- CodeMirror 源码保真编辑；只隐藏非当前结构的 Markdown 标记，不经过 HTML 反向转换。
- GFM、Frontmatter、表格、任务列表、脚注即时展示、数学公式与 Mermaid；点击或移动到当前结构时仍直接编辑原文。
- 约一秒自动保存，保存前比较磁盘哈希；外部修改不会被静默覆盖。
- 图片粘贴/拖入、内容哈希命名、相对链接和 `.marktree/config.json`。
- 工作区、暂存区、HEAD、远端、未保存内容及 Worktree 之间的对比。
- GitHub 设备授权接口，以及 GitLab、Gitee、自建 HTTPS Git 服务的令牌凭据。
- 中英文、亮暗主题和响应式桌面/手机布局。

## 内容与状态边界

Markdown 文件是内容唯一真源，Git 远程仓库负责跨设备同步。Marktree 的应用数据只保存最近仓库与文件、凭据引用、Marktree 修改清单、可恢复的 Git 操作阶段和冲突恢复副本，不保存 Markdown 正文镜像。

一键同步只提交 Marktree 自己保存或加入的 Markdown、图片和 `.marktree/config.json`。它使用独立 index 构造提交，混合仓库里的源码改动不会被纳入同步提交，用户原有暂存项也不会被改写。同步阶段会持久化，进程退出后能够继续，不会重复提交或重复应用临时保存的工作区状态。

仓库配置示例：

```json
{
  "assetsDir": "assets",
  "ignoreRules": ["build/**", "private/**"]
}
```

## 本地开发

需要 Node.js 24、Rust stable、Windows WebView2；Android 开发还需要 JDK 17、Android SDK、NDK 和 Git for Windows。仓库的 Android Rust 检查脚本会使用 `D:\Tools\GitPerlLib` 中的 OpenSSL 构建模块。

Windows 下可以直接双击根目录的 `start-dev.bat` 启动桌面开发版。脚本会优先使用 `D:\Tools` 中的 Node.js 与 Rust；如果尚未安装前端依赖，会先按 `package-lock.json` 执行 `npm ci`。

也可以在终端中手动启动：

```powershell
npm install
npm run build
npm run desktop
```

Android 开发工程由 Tauri 生成：

```powershell
npm run android:init
npm run android:dev
npm run android:build:debug
```

`npm run check:android-rust` 可以只检查 Android `aarch64` Rust 与原生依赖，不生成 APK。完整 Android 验收才运行本地 Debug 构建；不制作面向分发的 Release 包，也不发布应用商店版本。

GitHub 设备授权需要为自己的 OAuth App 提供构建环境变量：

```powershell
$env:MARKTREE_GITHUB_CLIENT_ID = "your-oauth-app-client-id"
npm run desktop
```

通用 HTTPS Git 服务可直接在仓库设置中保存用户名与个人访问令牌。凭据进入 Windows Credential Manager 或 Android Keystore，不写入仓库和应用状态 JSON。

## 验证

```powershell
npm test
npm run build
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml
```

运行 `npm run dev -- --host 127.0.0.1` 后，可用 `npm run test:visual` 检查桌面、紧凑桌面和手机布局。

Rust 集成测试使用真实临时仓库和裸远程仓库，覆盖分支与 Worktree、结构化 diff、外部写入保护、编码与换行保真、Marktree 路径隔离、图片同步、远端提交、第二设备推拉、失败重试和冲突恢复。前端测试覆盖混合换行、逐段三方合并及可见组件交互，不用消费端伪造的 Git 返回值替代核心链路。

更多实现说明见 [架构文档](docs/ARCHITECTURE.md) 和 [测试说明](docs/TESTING.md)。

## 明确不做

Marktree 不提供网页版、iOS、SSH、Bemo 数据迁移、AI、日历、标签体系、Obsidian 双链、交互式变基、拣选或 Git 标签管理。

## License

[MIT](LICENSE)
