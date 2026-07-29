export const demoContents: Record<string, string> = {
  'README.md': `---
title: Marktree
---

# 写作，像使用纸张一样自然

Marktree 直接编辑仓库里的 **Markdown 源文件**，并用 Git 在设备之间同步。

## 今日工作

- [x] 整理项目说明
- [ ] 完成新的文档

| 能力 | Windows | Android |
| --- | :---: | :---: |
| 多仓库 | ✓ | ✓ |
| 工作副本 | ✓ | — |
| 一键同步 | ✓ | ✓ |

公式保持原样：$E = mc^2$

\`\`\`mermaid
graph LR
  A[Markdown] --> B[Git]
  B --> C[其他设备]
\`\`\`
`,
  'docs/sync.md': '# 同步\n\n点击同步后，Marktree 只提交自己修改过的文档与资源。\n',
  'notes/ideas.md': '# 想法\n\n所有内容都只是普通文件。\n',
}
