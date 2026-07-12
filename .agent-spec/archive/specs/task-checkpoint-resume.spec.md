spec: task
name: "检查点与增量重跑（Checkpoint/Resume）"
inherits: project
tags: [done, bootstrap, lifecycle, verify, phase8]
depends: [task-goal-gate, task-context-fidelity]
estimate: 1d
---

## 意图

让 lifecycle 在多轮验证中跳过已通过的场景，只重跑失败和未覆盖的场景，
大幅缩短大型 spec 的迭代周期。支持两种策略：
增量模式（只跑失败的）和保守模式（全跑但检测回归）。

灵感来源：Attractor 的 checkpoint/resume 机制 + Autoresearch 的"成功保留 commit，失败 reset"模式。

## 已定决策

- checkpoint 保存到 `--run-log-dir` 指定目录下的 `.agent-spec/checkpoint.json`
- `--resume` 默认为增量模式：跳过上次 pass 的场景
- `--resume=conservative` 为保守模式：全部重跑，但对上次 pass 现在 fail 的标记回归
- checkpoint 记录每个场景的 verdict 和对应的 VCS ref
- 无 `--run-log-dir` 时 `--resume` 报错（checkpoint 需要持久化位置）

## 边界

### 允许修改
- src/spec_gateway/**
- src/spec_verify/**
- src/spec_core/**
- src/main.rs

### 禁止做
- 不要在无 `--run-log-dir` 时静默忽略 `--resume`
- 不要在增量模式下跳过 `fail` 或 `skip` 的场景
- 不要让 checkpoint 格式依赖特定 VCS 类型

## 完成条件

场景: 增量模式跳过已通过场景
  测试:
    包: agent-spec
    过滤: test_resume_incremental_skips_passed_scenarios
  假设 某个 checkpoint 记录场景 A 为 `pass`、场景 B 为 `fail`
  当 lifecycle 使用 `--resume` 执行
  那么 场景 A 不被重新执行
  并且 场景 B 被重新执行
  并且 最终报告包含场景 A 的 checkpoint verdict

场景: 保守模式检测回归
  测试:
    包: agent-spec
    过滤: test_resume_conservative_detects_regression
  假设 某个 checkpoint 记录场景 A 为 `pass`
  当 场景 A 本次重跑 verdict 为 `fail`
  那么 evidence 中包含 `regression: true` 标记

场景: 无 run-log-dir 时 resume 报错
  测试:
    包: agent-spec
    过滤: test_resume_without_run_log_dir_errors
  假设 lifecycle 未传入 `--run-log-dir`
  当 使用 `--resume` 参数
  那么 命令返回错误
  并且 错误信息提示需要 `--run-log-dir`

场景: checkpoint 文件可序列化和反序列化
  测试:
    包: agent-spec
    过滤: test_checkpoint_roundtrip_serialization
  假设 某次 lifecycle 产生了 checkpoint
  当 读取并反序列化该 checkpoint
  那么 所有场景的 verdict 和 VCS ref 被正确恢复
