# Agent CLI

`marktree-cli` 是 Marktree 的结构化本地接口，供 Agent 和自动化工具在不启动网页运行时的情况下读取、写入和同步工作区。CLI 与桌面端调用同一个 Rust `WorkspaceService`，文件内容仍只存在于用户选择的工作区中。

## 开发环境运行

当前项目不提供安装包。开发环境可以直接构建并运行：

```powershell
npm run cli -- --help
npm run cli -- workspace inspect --root "D:\Knowledge"
```

构建后的可执行文件位于 `src-tauri/target/debug/marktree-cli.exe`。CLI 默认使用与桌面端相同的应用状态目录；测试或隔离自动化可以传入全局参数 `--state-dir <目录>`。

`workspace inspect`、`document list/read/search`、`changes` 和 `sync plan` 是严格只读命令：它们不会创建状态目录或锁文件，不会迁移本地状态，也不会借读取之机恢复未完成操作。Windows 上除搜索外的这些入口通过按工作区身份命名的系统互斥量与桌面端和写入 CLI 协调，因此即使此前从未生成锁文件，也不会在文件事务中途读取；每次读取应用状态都会重新载入磁盘上的最新有效版本，不使用 CLI 启动时的旧快照。`document search` 是可取消、尽力而为的时间点扫描：它不会长时间阻塞写入，写入的原子发布和内部暂存隔离保证它只能看到完整旧文件或完整新文件，扫描中被移动或删除的文件会跳过。恢复只在 `workspace open`、写入、目录/移动和 `sync run` 这些明确会改动状态的命令进入时发生。因此，Agent 可以把只读命令用于探测和规划，而不改变工作区或 Marktree 状态。

## 输出合同

实际命令只向标准输出写一个 JSON 对象。成功结果：

```json
{"ok":true,"command":"document.read","data":{}}
```

参数、路径、版本或文件操作失败时返回非零退出码：

```json
{"ok":false,"command":"document.write","error":{"code":"externalChange","message":"The file changed outside Marktree."}}
```

Git 同步可能在提交、拉取或推送阶段失败。这时退出码同样非零，`error` 给出主要错误，`data` 保留同步阶段、提交 ID、精确路径和恢复状态。

## 命令

```text
workspace inspect --root <目录>
workspace open --root <目录>
document list --root <目录>
document read --root <目录> --path <相对路径>
document search --root <目录> --query <文本> [--limit 100]
document write --root <目录> --path <相对路径> (--expected-sha256 <哈希> | --expected-missing)
document write-batch
folder create --root <目录> --path <相对路径>
entry move --root <目录> --source <相对路径> --destination <相对路径>
changes --root <目录>
sync plan --root <目录>
sync run --root <目录>
```

`document write` 从标准输入读取完整正文。更新文件前先用 `document read` 取得 `sha256`，新建文件使用 `--expected-missing`；CLI 不提供无条件覆盖入口。版本比较与文件发布属于同一次条件提交：比较后到正式替换前发生的外部改写也会返回 `externalChange`，不会被静默覆盖。

`document write-batch` 从标准输入读取：

```json
{
  "root": "D:\\Knowledge",
  "writes": [
    {
      "path": "10-Knowledge/Marktree.md",
      "content": "# Marktree\n",
      "expectedSha256": "...",
      "encoding": "utf8"
    },
    {
      "path": "20-Sources/Marktree.md",
      "content": "# 来源\n",
      "expectedMissing": true,
      "encoding": "utf8"
    }
  ]
}
```

批次必须属于同一工作区，路径不能重复，每项必须声明当前哈希或确认文件不存在。Marktree 会先验证整批的路径和版本，再开始写入。

## 与 100x-learning 协同

知识库的目录语义仍由 `100x-learning` 和知识库自己的 `Home.md` 决定。Marktree 负责工作区边界、写入冲突、源码保真、恢复和可选 Git，不维护另一套知识分类。

在 `100x-learning` 源码目录配置一次 CLI：

```powershell
python scripts/private_library.py configure-marktree --cli "D:\Tools\Marktree\marktree-cli.exe"
python scripts/marktree_integration.py status
```

之后该 Skill 的持久化适配器会把正文、案例和索引写入交给 Marktree。普通工作区只写真实文件，不产生 Git 清单；只有工作区根目录自身是 Git 仓库时，CLI 写入才会成为 Marktree 的精确变更，并可由 `sync run` 提交和推送。
