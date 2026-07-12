spec: task
name: "标准化状态文件协议（Status File Contract）"
inherits: project
tags: [done, bootstrap, lifecycle, report, phase7]
depends: [task-goal-gate]
estimate: 0.5d
---

## 意图

为 lifecycle 提供一个面向机器消费的标准化状态文件输出，
让 CI/CD、GitHub Actions、其他 agent 等外部系统能通过固定协议读取验证结果，
与现有的 `--format json`（面向人类/agent 阅读）互补而非替代。

灵感来源：Attractor 的 `status.json` 节点输出协议。

## 已定决策

- 通过 `--status-file <path>` 参数触发写入
- 状态文件使用精简 JSON 格式，包含 `outcome` 高层摘要字段
- `outcome` 取值：`success` / `partial_success` / `fail` / `gate_blocked`
- 状态文件包含 `context_updates` 键值对，方便下游工具提取统计信息
- 不替代现有 `--format json` 输出，两者可同时使用

## 边界

### 允许修改
- src/spec_report/**
- src/main.rs

### 禁止做
- 不要修改现有 `--format json` 的输出结构
- 不要让 `--status-file` 成为必选参数
- 不要在状态文件中包含完整 evidence（那是 `--format json` 的职责）

## 完成条件

场景: 全部通过时写入 success 状态
  测试:
    包: agent-spec
    过滤: test_status_file_writes_success_on_all_pass
  假设 某个任务级 spec 所有场景 verdict 为 `pass`
  当 lifecycle 使用 `--status-file` 参数执行
  那么 状态文件中 `outcome` 为 `"success"`
  并且 `context_updates.tests_failing` 为 `0`

场景: 部分失败时写入 partial_success 状态
  测试:
    包: agent-spec
    过滤: test_status_file_writes_partial_success_on_mixed
  假设 某个任务级 spec 有 pass 和 fail 混合
  当 lifecycle 使用 `--status-file` 参数执行
  那么 状态文件中 `outcome` 为 `"partial_success"`
  并且 `context_updates.tests_passing` 和 `context_updates.tests_failing` 均大于 0

场景: gate_blocked 时 outcome 反映门禁状态
  测试:
    包: agent-spec
    过滤: test_status_file_outcome_reflects_gate_blocked
  假设 某个 critical 场景 fail
  当 lifecycle 使用 `--status-file` 参数执行
  那么 状态文件中 `outcome` 为 `"gate_blocked"`
  并且 `gate_blocked` 字段为 `true`

场景: 无 --status-file 时不产生文件
  测试:
    包: agent-spec
    过滤: test_no_status_file_flag_produces_no_file
  假设 lifecycle 未传入 `--status-file`
  当 lifecycle 执行完毕
  那么 不会在任何位置写入状态文件
