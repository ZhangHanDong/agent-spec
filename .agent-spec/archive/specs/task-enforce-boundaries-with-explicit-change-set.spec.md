spec: task
name: "对显式 change set 执行边界校验"
inherits: project
tags: [done, bootstrap, verify, boundaries, contract]
---

## 意图

让 `agent-spec` 能对显式提供的变更文件集合执行边界校验，
把 Task Contract 中的 `允许修改` / `禁止做` 路径规则变成真正的机械护栏。

## 已定决策

- `BoundariesVerifier` 只校验显式传入的 `change_paths`
- 如果存在允许列表，变更路径必须命中至少一个允许边界
- 命中禁止边界的路径直接判定失败

## 边界

### 允许修改
- src/spec_verify/**
- src/spec_gateway/**
- src/**
- specs/**

### 禁止做
- 不要伪造 git diff 或隐藏缺失基线的问题
- 不要把显式 change set 之外的路径当成已验证
- 不要让命中禁止边界的路径返回通过

## 完成条件

场景: 允许范围内的显式变更通过边界校验
  测试: test_boundaries_verifier_accepts_changes_within_allowed_paths
  假设 某个任务合约只允许修改 `src/spec_parser/**`
  当 verifier 检查显式变更路径 `crates/spec-parser/src/parser.rs`
  那么 边界校验结果为通过

场景: 超出允许范围的显式变更失败
  测试: test_boundaries_verifier_rejects_change_outside_allowed_paths
  假设 某个任务合约只允许修改 `src/spec_parser/**`
  当 verifier 检查显式变更路径 `crates/spec-gateway/src/lifecycle.rs`
  那么 边界校验结果为非通过
  并且 失败原因指出该路径不在允许边界内

场景: 命中禁止边界的显式变更失败
  测试: test_boundaries_verifier_rejects_change_matching_forbidden_boundary
  假设 某个任务合约允许修改 `src/spec_gateway/**`
  并且 同时禁止修改 `crates/spec-gateway/src/lib.rs`
  当 verifier 检查显式变更路径 `crates/spec-gateway/src/lib.rs`
  那么 边界校验结果为非通过
  并且 失败原因指出该路径命中了禁止边界
