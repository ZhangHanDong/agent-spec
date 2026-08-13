spec: task
name: "Guard 自动推导 staged change set"
inherits: project
tags: [done, bootstrap, cli, git, boundaries, guard]
---

## 意图

让 `agent-spec guard` 在 git 仓库中无需手工传入 `--change`，
也能从 staged index 自动推导变更文件集合，再交给边界校验使用。

## 已定决策

- 用户显式传入 `--change` 时优先使用显式值
- 未显式传入时，`guard` 尝试从 git staged index 读取变更路径
- 不在 git 仓库中时，`guard` 保持当前降级行为，不伪造 change set

## 边界

### 允许修改
- src/**
- specs/**

### 禁止做
- 不要要求用户在 pre-commit hook 里手工枚举所有 `--change`
- 不要覆盖用户显式传入的 `--change`
- 不要在非 git 目录里报出误导性的 staged change 错误

## 完成条件

场景: guard 从 staged git index 推导变更路径
  测试: test_resolve_guard_change_paths_reads_staged_git_changes
  假设 某个临时 git 仓库中存在 staged 文件 `src/lib.rs`
  当 `guard` 在未传入 `--change` 的情况下解析 change set
  那么 返回结果包含 `src/lib.rs`

场景: 显式 change 参数优先于 git 自动发现
  测试: test_resolve_guard_change_paths_prefers_explicit_changes
  假设 用户显式传入 `custom/file.rs`
  当 `guard` 解析 change set
  那么 返回结果使用显式传入的路径
  并且 不依赖 git staged index

场景: 非 git 目录保持空 change set
  测试: test_resolve_guard_change_paths_returns_empty_outside_git_repo
  假设 当前目录不是 git 仓库
  当 `guard` 在未传入 `--change` 的情况下解析 change set
  那么 返回空 change set
  并且 不会报 staged git 错误
