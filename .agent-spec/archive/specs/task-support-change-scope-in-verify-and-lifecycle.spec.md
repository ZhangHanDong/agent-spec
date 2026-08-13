spec: task
name: "verify 与 lifecycle 支持可选 change scope"
inherits: project
tags: [done, bootstrap, cli, git, boundaries, lifecycle, verify, phase4]
---

## 意图

让单任务验证入口也能直接消费 git 变更语义，
在需要边界校验时无需手工枚举 `--change`，同时保持当前默认行为稳定。

## 已定决策

- `verify` 与 `lifecycle` 新增 `--change-scope`
- 默认 scope 为 `none`，不自动推导 git 变更
- 显式 `--change` 继续拥有最高优先级

## 边界

### 允许修改
- src/**
- specs/**
- README.md

### 禁止做
- 不要把 `verify` 或 `lifecycle` 的默认行为改成自动读取 git 变更
- 不要让 `--change-scope` 覆盖显式 `--change`
- 不要让 `none` scope 在 git 仓库里偷偷读取 staged 或 worktree

## 完成条件

场景: lifecycle 在 worktree scope 下读取整棵工作区变更
  测试:
    包: agent-spec
    过滤: test_resolve_command_change_paths_reads_worktree_git_changes
  假设 某个临时 git 仓库同时存在 staged、未暂存和未跟踪变更
  当 `lifecycle` 使用 `worktree` change scope 解析 change set
  那么 返回结果包含这三类路径

场景: verify 默认 none scope 保持空 change set
  测试:
    包: agent-spec
    过滤: test_resolve_command_change_paths_returns_empty_for_none_scope
  假设 某个临时 git 仓库存在 staged 变更
  当 `verify` 使用默认 `none` scope 解析 change set
  那么 返回空 change set
  并且 不依赖 git 自动发现

场景: 显式 change 参数继续优先于自动 scope
  测试:
    包: agent-spec
    过滤: test_resolve_command_change_paths_prefers_explicit_changes
  假设 用户显式传入 `custom/file.rs`
  当 `verify` 同时配置 `worktree` scope
  那么 返回结果继续使用显式传入的路径
  并且 不依赖 git 自动发现
