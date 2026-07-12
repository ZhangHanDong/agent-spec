spec: task
name: "运行历史汇总视图"
inherits: project
tags: [done, bootstrap, lifecycle, report, phase8]
depends: [task-context-fidelity]
estimate: 0.5d
---

## 意图

为 lifecycle 的 run log 增加表格化汇总视图，
让人类和 agent 能快速判断多轮验证的趋势——哪些场景在改善，哪些在恶化。

灵感来源：Autoresearch 的 `results.tsv`（5 列，每行一个实验）。

## 已定决策

- 通过 `agent-spec lifecycle --run-log-dir <dir> --history` 触发
- 输出表格包含：运行序号、时间戳、pass/fail/skip/uncertain 计数、delta
- delta 列显示与上次运行的差异（如 `+2P -1F`）
- 支持 `--format json` 输出结构化历史数据

## 边界

### 允许修改
- src/main.rs
- src/spec_report/**

### 禁止做
- 不要修改现有 run log 文件格式
- 不要在 history 模式下重新执行验证

## 完成条件

场景: history 输出表格化汇总
  测试:
    包: agent-spec
    过滤: test_history_outputs_tabular_summary
  假设 `--run-log-dir` 中有 3 次运行记录
  当 lifecycle 使用 `--history` 参数
  那么 输出包含 3 行数据
  并且 每行包含 pass/fail/skip/uncertain 计数

场景: delta 列显示与前次差异
  测试:
    包: agent-spec
    过滤: test_history_delta_shows_diff_from_previous
  假设 第一次运行 2 pass 3 fail，第二次运行 4 pass 1 fail
  当 lifecycle 输出 history
  那么 第二行的 delta 显示 `+2` pass 和 `-2` fail

场景: 单次运行时 delta 为空
  测试:
    包: agent-spec
    过滤: test_history_single_run_no_delta
  假设 `--run-log-dir` 中只有 1 次运行记录
  当 lifecycle 输出 history
  那么 delta 列为空或显示 `—`

场景: history 支持 JSON 格式输出
  测试:
    包: agent-spec
    过滤: test_history_json_format_output
  假设 `--run-log-dir` 中有多次运行记录
  当 lifecycle 使用 `--history --format json` 参数
  那么 输出为 JSON 数组
  并且 每个元素包含 pass/fail/skip/uncertain 计数
