# Alexandrie 能力吸收审计

本审计针对用户提供的 `Alexandrie-main.zip` 完成。研究对象的前端版本为 8.11.0，ZIP 的 SHA-256 为 `C1BEFAECBEBF74CA4DA971A87FA44332C6FCDF767E2F84B99536E53EC0808F4E`。范围覆盖生产功能说明、前端页面与组件、编辑器扩展、状态和离线链、Go 路由与服务、数据模型与迁移、导入导出、备份、部署、测试和许可证；重复翻译、字体、图标、截图与纯样式细节不作为独立产品能力。

判断始终服从 Marktree 的边界：用户选择的普通文件夹是内容唯一真源；Git 只在工作区根目录明确具备仓库能力时负责历史和同步；Vue 负责交互，Rust/Tauri 负责工作区内文件、可选 Git、凭据和原生平台能力；不得引入 Web runtime、内容数据库、专有文档格式或 Alexandrie Node 兼容层。

## 已吸收并完成验收

| 优先级 | Alexandrie 用户结果 | Marktree 最终落点 | 真源与验收合同 |
| --- | --- | --- | --- |
| P1 | 统一命令中心 | `src/App.vue` 与 `src/components/WorkspaceOverlays.vue` 统一承载工作区切换、文件/正文/最近文件搜索，以及新建、设置、片段、刷新和同步命令；命令按平台、是否有工作区和精确根 Git 能力派生，并支持不受界面语言影响的稳定命令别名。 | 搜索结果由 Rust 读取真实文件产生；普通工作区没有同步命令，Git 演示与真实 Git 工作区才出现同步。视觉检查执行两种上下文。 |
| P1 | 带上下文的全文搜索 | `src-tauri/src/documents/catalog.rs` 返回路径、真实行列、片段、命中类型、文件类型和修改时间；命令中心提供目录、类型和时间筛选及命中高亮。 | 不建立索引数据库。Rust 测试从真实磁盘生产命中，Vue 只展示结果。 |
| P1 | 移动端 Markdown 工具栏 | `src/components/MarkdownToolbar.vue` 与 CodeMirror 事务实现标题、链接、列表、任务、代码、表格、公式和附件入口。 | 每项操作都直接修改当前原始 Markdown 字符串；组件测试在真实 CodeMirror 缓冲区验证。 |
| P1 | 可管理的 Markdown 片段 | `src/lib/editor/snippets.ts`、`src/components/SnippetManager.vue` 提供增删改、工具栏与 `/快捷词` 补全，以及带版本的 JSON 导入导出。 | 只进入个人本机设置，不进入工作区；内置片段只使用通用 Markdown，导入拒绝无效结构。 |
| P1 | Android 系统分享入口 | `src-tauri/src/android_bridge.rs`、`src-tauri/src/portability.rs`、`src/components/AndroidShareDialog.vue` 接收文字、网址、Markdown、图片、普通附件和 ZIP；用户选择真实工作区、目录和是否插入当前文档。 | 原生桥只搬运一次性 Intent；分类、命名、写入、相对链接和精确 Git 变更统一进入 `WorkspaceService`。文字插入不会再创建第二个文件，附件保留原字节。 |
| P1 | Android 工作区导入导出 | `src-tauri/src/archive.rs` 和工作区设置实现普通工作区 ZIP 原样导入/导出。 | 保留文件字节、空目录和 `.marktree/config.json`，排除 `.git` 与应用内部状态；路径穿越、重复项、符号链接、体积超限和任意层级 Git 元数据在提交前失败。 |
| P2 | 普通相对链接补全 | `src/components/MarkdownEditor.vue` 从当前工作区条目生成相对 Markdown 链接补全。 | 链接只使用普通相对路径，不引入数字节点 ID、双链或链接数据库。 |
| P2 | 选择已有图片或附件 | 编辑器附件选择器消费当前真实工作区文件并插入标准图片或文件链接。 | 不复制文件、不建立资源库；文档仍只保存普通 Markdown。 |
| P2 | PDF、音频、视频预览 | `read_workspace_preview` 在 Rust 边界校验路径、类型和大小，Vue 对图片、PDF、音频、视频建立临时预览；其他类型调用系统程序。 | Rust 测试直接比较四类真实文件的原字节，预览状态不持久化。 |
| P2 | 文件复制/创建副本 | `duplicate_workspace_entry`、目录树上下文菜单和持久化工作区操作协议支持文件或目录副本。 | 完整复制计划持久化全部源、目标、哈希与空目录；中途退出后补齐，Git 工作区只登记目标文件的精确 `Upsert`。文件系统调用的模糊错误只产生一个终态。 |
| P2 | 编辑体验偏好 | `src/lib/editor/preferences.ts` 与 `src/components/EditorPreferencesPanel.vue` 提供字体、字号、拼写检查、字数/行数、舒适/紧凑密度和侧栏宽度。 | 只保存有范围上限的个人 UI 状态，不进入工作区和正文。 |
| P3 | 标签页恢复与收藏 | `src/lib/workspaceUiState.ts` 按工作区保存标签路径、当前路径和收藏路径；启动时经正式文档打开器重新读取真实文件。 | 状态不含正文。移动、回收和移除工作区统一迁移或清理路径；不存在的文件退出恢复集合。 |
| P3 | 打印与只读阅读视图 | `src/components/EditorWorkspace.vue` 和打印样式从当前 CodeMirror 投影临时阅读或打印。 | 不保存编译 HTML，不建立第二份内容。 |
| P3 | 更完整的文档导航 | 使用真实 Markdown 语法树生成层级大纲，另有可点击路径面包屑和上一篇/下一篇。 | 代码围栏中的伪标题不会进入大纲；定位回到当前原始编辑缓冲区。 |

这些能力的公共边界、持久化规则和真实链测试分别记录在 `docs/ARCHITECTURE.md` 与 `docs/TESTING.md`。它们不是 Alexandrie 代码的移植，而是按 Marktree 的 `WorkspaceService → 真实文件/路径型 UI 状态 → Vue 消费端` 边界重新实现。

## 已有能力，只补齐交互或验证

| 能力 | 判断 | 原因 |
| --- | --- | --- |
| 标准 Markdown、GFM、表格、任务、脚注、数学、Mermaid | 保留 Marktree 实现 | Marktree 已按源码保真实现，不能换成 Alexandrie 的 Markdown-it 编译 HTML 与数据库正文链。 |
| 最近文档、多标签页 | 增强 | 最近文件和标签原已存在；本轮补齐命令中心排序、跨重启路径恢复和路径移动合同。 |
| 标题跳转和文件路径 | 增强 | 原有基础导航继续扩展为层级大纲、可点击面包屑和前后文档。 |
| 图片粘贴与内容哈希资源 | 增强 | 保留现有单一资源真源，仅补充选择工作区已有文件和普通附件链接。 |
| 中英文与 i18n 架构 | 保留扩展能力 | 不因为 Alexandrie 有更多翻译就预装没有真实用户结果的语言；新增功能完整接入现有中英文合同。 |

## 明确拒绝或暂不吸收

| Alexandrie 能力 | 结论 | 拒绝理由或未来正确方向 |
| --- | --- | --- |
| 自定义容器、彩色块、卡片、面板、Tooltip、上下标等扩展语法 | 拒绝 | 会形成 Marktree 专属方言，降低其他编辑器可读性。未知语法继续显示原文。 |
| Alexandrie 默认的 `:::card`、`:::columns` 等片段 | 拒绝 | 片段只能辅助输入通用 Markdown，不能借片段偷偷引入专属格式。 |
| 浏览器 SpeechRecognition 语音输入 | 暂不吸收 | 来源依赖实验性浏览器接口并硬编码 `fr-FR`，无法形成 Windows/Android 一致合同；以后应单独研究系统级听写。 |
| Draw.io 内嵌编辑 | 拒绝 | 依赖 `embed.diagrams.net` 外部网页与跨窗口消息；`.drawio` 或 SVG 应继续作为普通工作区文件交给系统工具。 |
| 数字 ID 内部链接 | 拒绝 | `#726…` 依赖数据库身份。Marktree 只提供标准相对文件链接，不增加节点 ID、双链或链接索引库。 |
| 独立 CDN/S3 文件管理器 | 拒绝 | 工作区文件已经是资源真源，再建对象存储会产生第二套生命周期。 |
| 列表、表格、卡片式文件浏览 | 暂不吸收 | 真实文件树已忠实表达目录，目前没有用户任务证明需要第二种主要文件浏览模型。 |
| Kanban | 拒绝 | Alexandrie 把列与卡片归属写入节点 metadata，会制造文件之外的任务真源并把产品扩成任务管理器。 |
| 团队、工作区、分类、文档、资源统一 Node 模型 | 拒绝 | Marktree 的身份来自真实工作区、目录和文件；统一数据库节点会直接推翻核心架构。 |
| 标签、缩略图、颜色、每文档主题、自定义排序 | 拒绝来源模型 | 这些字段在 Alexandrie 数据库而非 Markdown 文件。若未来确有需求，只能先单独决定是否采用标准 Frontmatter。 |
| PWA 页面/API/CDN 缓存 | 拒绝 | Marktree 是原生本地应用，天然离线，不需要 Web 缓存，也不得新增 Web runtime。 |
| 浏览器离线增量队列 | 拒绝 | Marktree 已有真实文件写入、版本检查、持久化操作恢复和可选 Git 同步；复制队列会形成第二套不一致状态。 |
| Windows Markdown/目录导入 | 拒绝 | Windows 直接打开真实文件夹优于复制导入。Android 只做普通工作区迁入，且不解析或改写正文。 |
| Frontmatter 提取与换行规范化 | 拒绝 | 会改变源文件；Alexandrie 的 `normalizeLineEndings` 选项在研究源码中也没有形成可靠执行合同。 |
| 服务器备份任务与节点 JSON 备份 | 拒绝实现 | Marktree 的移动结果是普通工作区精确 ZIP，不需要节点 JSON、设置 JSON、S3 文件和临时下载链接。 |
| “导出 Markdown” | 不需要 | 工作区文件本身已经是 Markdown；复制路径、系统打开、系统分享和工作区 ZIP 才是正确用户结果。 |
| 公共链接、SEO 页面、一键发布 | 拒绝 | 需要 Web 服务和公共内容边界，违反原生本地架构。 |
| 团队权限、邀请、公开/私有访问 | 拒绝 | Marktree 是本地文件工作区；跨设备历史与同步只由用户明确启用的 Git 承担。 |
| 账户、密码重置、OIDC、2FA、会话、管理员 | 拒绝 | 对本地 Windows/Android 工作台没有对应用户结果。 |
| 管理员统计和团队趋势 | 拒绝 | 依赖中央数据库与多用户行为收集。 |
| 自定义 CSS 注入和每文档视觉主题 | 拒绝 | 容易制造不可维护的界面和专属表现；只保留有限且有边界的个人 UI 偏好。 |
| Docker、MySQL、S3、SMTP、反向代理部署 | 拒绝 | 与 Marktree 原生本地生命周期无共享边界。 |
| “实时协作、评论、活动历史、分析、自定义域名、密码保护” | 不列为已实现候选 | 这些只出现在 Alexandrie 营销文案，研究到的生产模型、接口和实时通信链没有对应实现，不能把声明当成可吸收证据。 |

## 代码与许可证结论

Alexandrie 把正文和编译 HTML 一起放入数据库，Marktree 则必须让普通文件保持唯一真源；两者正确性边界相反。Alexandrie 的离线增量、节点身份、权限和备份实现都不能作为 Marktree 的兼容层保留。本轮没有复制 Alexandrie 的实质性代码，也没有引入 Alexandrie 依赖，因此无需在 Marktree 源文件中增加其 MIT 版权头。若未来确实复制实质性代码，必须同时保留 Alexandrie 的 MIT 版权与许可声明，并重新核对相应依赖许可证。

最终状态是“吸收用户结果，不吸收冲突架构”：上表 P1、P2、P3 项均已落到 Marktree 自身边界并完成自动化或可见链验收；所有拒绝项仍未进入内容模型、IPC、持久化状态或产品界面。
