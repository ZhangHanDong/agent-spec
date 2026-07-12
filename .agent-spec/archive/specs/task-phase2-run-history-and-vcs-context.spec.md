spec: task
name: "Phase 2：Run History 与 VCS Context"
inherits: project
tags: [done, roadmap, planned, phase2, traceability]
---

## 意图

让 reviewer 和作者不仅看到“这次 pass/fail”，
还看到 Contract 是如何被迭代到通过的，以及该结果与当前 VCS 上下文如何关联。

## 已定决策

- run log 先采用本地 sidecar 目录 `.agent-spec/runs/`
- explain 通过 `--history` 消费 run log，而不是复制 lifecycle 逻辑
- VCS 上下文自动检测 Git / jj / none
- `--change-scope jj` 作为 VCS-aware 扩展，但不改变 Git 的默认路径

## 边界

### 允许修改
- src/**
- src/spec_gateway/**
- src/spec_report/**
- README.md
- .gitignore
- specs/**

### 禁止做
- 不要一上来把完整 Agent 对话写入 run log
- 不要让 jj 支持破坏现有 Git 默认行为
- 不要要求用户必须提交 `.agent-spec/runs/`

## 完成条件

场景: lifecycle 可记录结构化 run log
  测试:
    包: agent-spec
    过滤: test_lifecycle_writes_structured_run_log_summary
  假设 用户显式启用 run log 记录
  当 lifecycle 执行结束
  那么 `.agent-spec/runs/` 下生成结构化运行记录
  并且 记录包含 verdict、criteria summary 与 VCS 信息

场景: explain 展示执行历史
  测试:
    包: agent-spec
    过滤: test_explain_history_reads_run_log_summary
  假设 某个 Contract 已经积累多次运行记录
  当 用户运行 `agent-spec explain --history`
  那么 输出包含 runs、first pass 与失败轨迹摘要
  并且 reviewer 能看出是否经历了多轮重试

场景: 命令行支持 jj change scope
  测试:
    包: agent-spec
    过滤: test_resolve_command_change_paths_reads_jj_changes
  假设 当前工作区是 jj 仓库
  当 用户使用 `--change-scope jj`
  那么 verify、lifecycle 或 guard 能解析 jj 的变更路径
  并且 Git 路径的默认行为保持不变

## 排除范围

- org.spec 组织级治理
- AI 对抗性验证
