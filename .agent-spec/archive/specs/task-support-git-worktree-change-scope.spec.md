spec: task
name: "Guard 支持 git worktree change scope"
inherits: project
tags: [done, bootstrap, cli, git, boundaries, guard, phase4]
---

## 意图

让 `agent-spec guard` 在需要更强边界校验时，
可以从整个 git worktree 推导 change set，而不只局限于 staged index。

## 已定决策

- `guard` 新增 `--change-scope`，首批支持 `staged` 与 `worktree`
- 默认 scope 仍然是 `staged`，保持 pre-commit 语义稳定
- `worktree` scope 包含 staged、未暂存和未跟踪文件

## 边界

### 允许修改
- src/**
- specs/**
- README.md

### 禁止做
- 不要改变默认 `guard` 行为为 worktree
- 不要让显式 `--change` 失去最高优先级
- 不要在 worktree 模式下漏掉未跟踪文件

## 完成条件

场景: worktree scope 包含 staged、未暂存和未跟踪文件
  测试:
    包: agent-spec
    过滤: test_resolve_guard_change_paths_reads_worktree_git_changes
  假设 某个临时 git 仓库同时存在 staged、未暂存和未跟踪变更
  当 `guard` 使用 `worktree` change scope 解析 change set
  那么 返回结果包含这三类路径

场景: 默认 staged scope 不包含未暂存改动
  测试:
    包: agent-spec
    过滤: test_resolve_guard_change_paths_ignores_unstaged_changes_in_default_staged_scope
  假设 某个临时 git 仓库存在 staged 和未暂存改动
  当 `guard` 使用默认 `staged` scope 解析 change set
  那么 返回结果只包含 staged 路径
  并且 未暂存改动不会被纳入

场景: 显式 change 参数优先于 scope 自动发现
  测试:
    包: agent-spec
    过滤: test_resolve_guard_change_paths_prefers_explicit_changes
  假设 用户显式传入 `custom/file.rs`
  当 `guard` 同时配置 `worktree` scope
  那么 返回结果继续使用显式传入的路径
  并且 不依赖 git 自动发现
