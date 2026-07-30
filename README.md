# Marktree

Marktree 是一个面向普通用户的 Windows / Android 本地 Markdown 与文本文件工作台。任何普通文件夹都能直接作为工作区打开；磁盘文件始终是内容唯一真源，不需要先建立 Git 仓库，也不需要导入专有格式。Git 是可选的高级能力，只在工作区根目录明确启用时负责历史与跨设备同步。

当前仓库是源码开发阶段，不提供安装包或应用商店版本。

## 已实现

- Vue 3、TypeScript 与 Tauri 2 共用 Windows / Android 界面和领域模型。
- Windows 可打开任意文件夹，使用可展开目录树新建、重命名、移动、回收站删除文件和文件夹，并调用系统程序打开其他文件。
- Markdown 和受支持 UTF-8 编码的纯文本可编辑；图片内置预览，其他文件交给系统程序。
- Android 在应用私有目录创建普通工作区，提供应用内废纸篓、恢复与清空，同时保留可选的 Git 克隆和同步。
- Rust `git2/libgit2` 提供可选 Git，无需系统安装 Git；仅精确识别工作区根目录，不向父目录发现仓库。
- Git 工作区支持 Worktree、独立窗口与日常 Git 操作；主界面只显示同步和简短状态，其余操作集中在“高级 Git”。
- CodeMirror 源码保真编辑；Markdown 模式只隐藏非当前结构的标记，纯文本模式关闭 Markdown 装饰，不经过 HTML 反向转换。
- GFM、Frontmatter、表格、任务列表、脚注即时展示、数学公式与 Mermaid；点击或移动到当前结构时仍直接编辑原文。
- 约一秒自动保存，保存前比较磁盘哈希；外部修改不会被静默覆盖。
- 图片粘贴/拖入、内容哈希命名、相对链接和 `.marktree/config.json`。
- 工作区、暂存区、HEAD、远端、未保存内容及 Worktree 之间的对比。
- GitHub 设备授权接口，以及 GitLab、Gitee、自建 HTTPS Git 服务的令牌凭据。
- 中英文、亮暗主题和响应式桌面/手机布局。

## 内容与状态边界

工作区根目录是文件访问、监听、搜索和保存的唯一边界，磁盘文件是内容唯一真源。Marktree 的应用数据只保存最近工作区与文件、界面状态、凭据引用、可选 Git 的精确变更清单、可恢复操作阶段和冲突恢复副本，不保存 Markdown 或文本正文镜像。

普通工作区不会调用 Git，也不会生成 Git 变更清单。启用版本管理时，Marktree 先预览当前可见文件、大小和忽略摘要，随后初始化 `main` 并建立一次完整本地基线提交。基线之后，一键同步只提交由 Marktree 成功创建、编辑、移动或删除的精确路径；外部程序产生的其他改动不会被自动纳入。同步使用独立 index，用户原有暂存项不会被改写，持久化阶段可在进程退出后安全继续。

工作区配置示例：

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

通用 HTTPS Git 服务可在已启用 Git 的工作区设置中保存用户名与个人访问令牌。凭据进入 Windows Credential Manager 或 Android Keystore，不写入工作区和应用状态 JSON。

## 验证

```powershell
npm test
npm run build
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml
```

运行 `npm run dev -- --host 127.0.0.1` 后，可用 `npm run test:visual` 检查桌面、紧凑桌面和手机布局。

Rust 测试使用真实普通目录、嵌套仓库、Git 仓库和裸远端，覆盖普通工作区文件链、精确根目录 Git 判断、基线提交、分支与 Worktree、结构化 diff、外部写入保护、编码与换行保真、精确路径同步、第二设备推拉、失败重试和冲突恢复。前端测试覆盖普通/Git 界面分流、混合换行、逐段三方合并及可见组件交互，不用消费端手写 Git 返回值替代核心链路。

更多实现说明见 [架构文档](docs/ARCHITECTURE.md) 和 [测试说明](docs/TESTING.md)。

## 明确不做

Marktree 不提供网页版、iOS、Android SAF、父工作区内多仓库统一操作、任意二进制编辑、SSH、Bemo 数据迁移、AI、日历、标签体系、Obsidian 双链、交互式变基、拣选或 Git 标签管理。

## License

[MIT](LICENSE)
