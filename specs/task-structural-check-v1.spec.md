spec: task
name: "StructuralRule v1：机械分层/禁止引用检查"
inherits: project
tags: [probe, structural, phase7]
depends: [task-probe-abstraction-v1]
estimate: 2d
---

## 意图

实现 Probe 抽象里第一个非 Test 的机械探针:结构检查(dependency-cruiser 的轻量等价)。
给定代码路径,机械检测"匹配某 glob 的文件中是否出现被禁止的引用/模式",用于强制分层
(例如 clients 层不得 import services 层)。这是 Phase 7 中唯一可在本环境机械自验证的部分;
跨语言 test runner 与 NFR 外部探针(criterion/k6)需要真实外部工具,留待后续。

## 已定决策

- 纯函数 `structural_violations(code_paths, forbidden, file_glob) -> Vec<String>`:返回匹配 `file_glob` 且内容包含 `forbidden` 子串的文件路径列表。
- 文件遍历跳过 `target/` 与 `.git/`;glob 用简单 `*`/`**` 后缀/包含匹配。
- 新命令 `agent-spec check-structure --code <dir> --forbid <substr> --in <glob>`:打印违规文件;有违规时非零退出。
- 该检查对应 `Probe::Static` 的执行语义(Phase 6.5 已预留该变体);本期提供执行器与命令,DSL 解析与 lifecycle 集成留待后续。
- 不改 `is_passing` / 现有 verification。

## 边界

### 允许修改

- src/spec_report/**（structural 模块)
- src/main.rs（check-structure 子命令)
- README.md、examples/**

### 禁止做

- 不要在本期把 structural 检查接入 lifecycle/guard 门禁或 Scenario.probe(留待后续)。
- 不要实现跨语言 test runner 或 NFR 外部探针执行(需真实外部工具)。
- 不要改 `is_passing` / TestVerifier。

## 完成条件

### Rule: structural-detects-forbidden-reference — 机械检测被禁止的引用

场景: 匹配文件含禁止子串时报违规
  测试:
    过滤: test_structural_flags_forbidden_reference
  假设 代码目录下 `clients/a.rs` 含 `use crate::services::X;`,glob 为 `clients/**`,禁止子串为 `crate::services`
  当 调用 `structural_violations`
  那么 返回的违规列表包含 `clients/a.rs`

场景: 无违规时返回空
  测试:
    过滤: test_structural_no_violation_returns_empty
  假设 匹配文件都不含禁止子串
  当 调用 `structural_violations`
  那么 返回空列表

### Rule: glob-scoping — 只检查匹配 glob 的文件

场景: glob 之外的文件不被检查
  测试:
    过滤: test_structural_respects_glob_scope
  假设 `services/b.rs` 含禁止子串,但 glob 为 `clients/**`
  当 调用 `structural_violations`
  那么 `services/b.rs` 不在违规列表中

场景: 跳过 target 目录
  测试:
    过滤: test_structural_skips_target_dir
  假设 `target/x.rs` 含禁止子串
  当 调用 `structural_violations` 且 glob 为 `**`
  那么 `target/x.rs` 不在违规列表中

### Rule: command-exit-semantics — 命令有/无违规的退出语义

场景: 有违规时报告非空
  测试:
    过滤: test_check_structure_reports_violations
  假设 存在至少一个违规文件
  当 运行结构检查核心
  那么 违规列表非空(命令将以非零状态退出)

## 排除范围

- 跨语言 test runner(非 cargo 的 runner 执行)—— 需真实外部工具
- NFR 探针执行(criterion 基准、k6 负载、HTTP 探针)—— 需真实外部工具
- 完整 import 图分析(circular import、传递依赖)—— 本期只做"禁止子串 in glob"
- 把 structural 检查接入 lifecycle/guard 门禁或 Scenario.probe DSL —— 后续
