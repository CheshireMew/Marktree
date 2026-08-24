# Marktree

Marktree 是一个面向普通用户的 Windows / Android 本地 Markdown 与文本文件工作台。任何普通文件夹都能直接作为工作区打开；磁盘文件始终是内容唯一真源，不需要先建立 Git 仓库，也不需要导入专有格式。Git 是可选的高级能力，只在工作区根目录明确启用时负责历史与跨设备同步。

当前仓库是源码开发阶段，不提供安装包或应用商店版本。

## 已实现

- Vue 3、TypeScript 与 Tauri 2 共用 Windows / Android 界面和领域模型。
- Windows 可打开任意文件夹，使用可展开目录树新建、重命名、移动、创建副本、收藏和回收站删除文件或文件夹，并调用系统程序打开其他文件。
- Markdown 和受支持 UTF-8 编码的纯文本可编辑；图片、PDF、音频和视频可在应用内只读预览，其他文件交给系统程序。
- Android 在应用私有目录创建普通工作区，提供应用内废纸篓、恢复与清空、普通工作区 ZIP 原样导入/导出，以及来自系统分享面板的文字、Markdown、图片、一般附件和 ZIP 接收。导入前由用户选择真实工作区和目标目录，也可把文字或附件链接插入当前 Markdown；Git 克隆和同步仍是可选能力。
- Rust `git2/libgit2` 提供可选 Git，无需系统安装 Git；仅精确识别工作区根目录，不向父目录发现仓库。
- Git 工作区支持 Worktree、独立窗口与日常 Git 操作；主界面只显示同步和简短状态，其余操作集中在“高级 Git”。
- CodeMirror 源码保真编辑；Markdown 模式只隐藏非当前结构的标记，纯文本模式关闭 Markdown 装饰，不经过 HTML 反向转换。
- GFM、Frontmatter、表格、任务列表、脚注即时展示、数学公式与 Mermaid；点击或移动到当前结构时仍直接编辑原文。
- `Ctrl/Cmd+P` 打开统一命令中心，可切换工作区、执行当前平台与工作区确实可用的命令，并按路径或正文搜索全部已打开工作区。搜索可限制目录、Markdown/纯文本和修改时间，结果高亮真实上下文并保留行列，选择后直接定位原文。
- 手机编辑器提供标题、链接、列表、任务、代码、表格、公式和现有文件等 Markdown 工具；输入普通链接时可补全工作区相对路径。个人片段支持新增、修改、删除、导入和导出，可通过工具栏或 `/快捷词` 插入；片段只保存通用 Markdown，不改变文档格式。
- 标签页可按工作区跨重启恢复，收藏、最近文件和恢复信息只保存路径，不缓存正文；路径移动或回收时，这些界面状态与真实文件操作一起迁移。
- 编辑区提供层级大纲、可点击路径、上一篇/下一篇、阅读视图、打印、字数/行数统计，以及字体、字号、拼写检查、界面密度和侧栏宽度偏好。阅读与打印都从当前原始 Markdown 临时呈现，不保存编译 HTML。
- 约一秒自动保存，保存前比较磁盘哈希；外部修改不会被静默覆盖。
- 图片粘贴/拖入、内容哈希命名、相对链接和 `.marktree/config.json`。
- 工作区、暂存区、HEAD、远端、未保存内容及 Worktree 之间的对比。
- GitHub 设备授权接口，以及 GitLab、Gitee、自建 HTTPS Git 服务的令牌凭据。
- 中英文、亮暗主题和响应式桌面/手机布局。

## 内容与状态边界

工作区根目录是文件访问、监听、搜索和保存的唯一边界，磁盘文件是内容唯一真源。Marktree 的应用数据只保存最近工作区与文件路径、标签页和收藏路径、个人输入与界面偏好、凭据引用、可选 Git 的精确变更清单、可恢复操作阶段、冲突恢复副本，以及不含正文和凭据的有界操作日志，不保存 Markdown 或文本正文镜像。

普通工作区不会调用 Git，也不会生成 Git 变更清单。启用版本管理时，Marktree 先预览当前版本化投影、大小和忽略摘要，随后初始化 `main`，把普通内容与隐藏的 `.marktree/config.json` 一起建立一次完整本地基线提交。文件写入、移动、回收和 Git 初始化都采用可恢复的持久化操作协议；文件系统效果与整批 Git 变更不会停留在半迁移状态。基线之后，一键同步只提交由 Marktree 成功创建、编辑、移动或删除的精确路径；外部程序产生的其他改动不会被自动纳入。同步使用独立 index，用户原有暂存项不会被改写，持久化阶段可在进程退出后安全继续。

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

`npm run check:android-rust` 编译 Android `aarch64` Rust 与原生依赖，`npm run check:android-native` 先让 Tauri 注册自定义 Android 桥接插件，再编译应用和插件 Kotlin、合并 Arm 与 Universal 两种 Debug 清单，并检查最终应用确实包含分享入口、桥接插件类和唯一的 FileProvider。两项检查都不生成 APK。完整 Android 验收才运行本地 Debug 构建；不制作面向分发的 Release 包，也不发布应用商店版本。

## Agent CLI

源码开发版同时提供 `marktree-cli`。它使用稳定 JSON 输入输出，并与桌面端共用工作区写入、外部修改保护、恢复协议和精确 Git 变更清单：

```powershell
npm run cli -- workspace inspect --root "D:\Knowledge"
Get-Content note.md -Raw | npm run cli -- document write --root "D:\Knowledge" --path "note.md" --expected-sha256 <sha256>
```

CLI 支持工作区检查、列表、读取、搜索、带版本保护的单文件与批量写入、目录创建、移动、变更查询和一键同步。检查、列表、读取、搜索、变更查询和同步计划是严格只读命令，不会创建状态、触发迁移或恢复未完成操作；写入的版本比较与原子发布则处于同一条件提交边界。完整合同和 `100x-learning` 接入方式见 [Agent CLI 文档](docs/CLI.md)。

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
npm run test:cli
npm run check:windows-native
npm run check:android-rust
npm run check:android-native
```

运行 `npm run dev -- --host 127.0.0.1` 后，可用 `npm run test:visual` 检查桌面、紧凑桌面和手机布局。

Rust 测试使用真实普通目录、嵌套仓库、Git 仓库和裸远端，覆盖普通工作区文件链、有读取总量和单文件上限的结构化搜索、按需媒体资源读取、分块资源上传、创建副本及中断恢复、ZIP 原样往返、带生命周期清理的系统分享导入、精确根目录 Git 判断、版本化配置基线、持久化边界崩溃恢复、分支与 Worktree、结构化 diff、外部写入保护、编码与换行保真、精确路径同步、第二设备推拉、失败重试和冲突恢复。`check:windows-native` 还会真实启动两次 Tauri：第一次通过编辑器和关闭拦截保存，第二次从编辑器读回、渲染并再次保存。前端测试覆盖按根目录分流能力、普通/Git 界面层级、命令搜索、手机 Markdown 工具栏、相对链接、片段导入导出、标签恢复和收藏迁移、导航与阅读、媒体预览、Android 目标选择、操作日志消费、混合换行、逐段三方合并及可见组件交互，不用消费端手写返回值替代正在验证的核心链路。

更多实现说明见 [架构文档](docs/ARCHITECTURE.md)、[测试说明](docs/TESTING.md) 和 [Alexandrie 能力吸收审计](docs/ALEXANDRIE_CAPABILITY_AUDIT.md)。

## 明确不做

Marktree 不提供网页版、iOS、通用 Android SAF 文件系统访问、父工作区内多仓库统一操作、任意二进制编辑、SSH、Bemo 数据迁移、AI、日历、标签体系、Obsidian 双链、交互式变基、拣选或 Git 标签管理。Android 的系统分享与 ZIP 导入只把外部内容复制成应用私有目录里的普通工作区，不把外部目录变成持久文件系统边界。

## License

Marktree 当前原创源码采用 [AGPL-3.0-or-later](LICENSING.md)。截至 `157b713f9730b71ad0c13e123b19486b60ab843a` 的版本仍保留其 GPL-3.0-or-later 授权，截至 `97f3675fd124f81513721f7d044001d19c010486` 的版本仍保留其 MIT 授权。
