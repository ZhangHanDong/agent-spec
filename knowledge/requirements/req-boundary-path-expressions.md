---
kind: requirement
id: REQ-BOUNDARY-PATH-EXPRESSIONS
title: "Boundary Path Expressions"
status: proposed
liveness: auto
tags: [boundaries, verification, parser, lint]
---

# Boundary Path Expressions

## Problem

`### Allowed Changes` 条目今天要先通过一个"看起来像路径"的白名单（含 `/`、
`*` 或以 `.rs .ts .js .py .md .spec` 结尾）才会成为边界模式。裸根目录文件
（`Cargo.toml`、`LICENSE`、`Makefile`）、`.json/.toml/.yml/.txt` 等后缀，以及
带反引号的 `` `CLAUDE.md` `` 都被静默丢弃；结果是作者明明声明了允许，变更
仍被判"not covered by any allowed boundary"。带尾注的条目
（`` `Cargo.toml` (dev-dep only) ``）整串成为永不匹配的活模式。同一份识别与
规范化逻辑在 `spec_verify/boundaries.rs`、`spec_gateway/plan.rs`、
`spec_mcp/tools.rs` 各有一份且互不一致：`./Cargo.toml` 在前两处能过，在 MCP
处零规范化反而不匹配。robrix2 实践反馈（2026-08）把它列为 P0。

## Requirements

[REQ-BOUNDARY-PATH-EXPRESSIONS-ALLOW-IS-PATH] `### Allowed Changes` 下的每个条目 MUST 按定义视为路径表达式并在规范化后参与匹配。

[REQ-BOUNDARY-PATH-EXPRESSIONS-NO-SILENT-DROP] 工具 MUST NOT 用后缀或分隔符白名单静默丢弃 Allow 条目。

[REQ-BOUNDARY-PATH-EXPRESSIONS-NORMALIZE] 边界模式规范化 MUST 依次剥离反引号、剥离尾注、把 `\` 换成 `/`、去掉前导 `./` 与首尾 `/`，并把裸文件名视为仓库根相对路径。

[REQ-BOUNDARY-PATH-EXPRESSIONS-NOTE] 尾注 MUST 只识别反引号跨度之外、前有空白的 ` — note` 与 ` # note` 两种形式。

[REQ-BOUNDARY-PATH-EXPRESSIONS-PAREN-KEPT] 条目中的 `(…)` MUST NOT 被当作尾注剥离。

[REQ-BOUNDARY-PATH-EXPRESSIONS-DENY-TOKEN] `### Forbidden` 与无子标题的 General 条目 MUST 只在规范化后为单个路径 token（不含空白且含 `/`、`*`、`.` 或 `?` 之一）时成为禁止模式。

[REQ-BOUNDARY-PATH-EXPRESSIONS-NO-PROSE-EXTRACT] 自然语言禁令中的反引号路径 MUST NOT 被抽取为禁止模式。

[REQ-BOUNDARY-PATH-EXPRESSIONS-SHAPE-LINT] 规范化后为空，或含空白且首尾 token 之一不含路径字符（`/`、`.`、`*`）的 Allow 条目 MUST 触发 Warning 级 lint `boundary-entry-shape`，其消息包含条目文本并建议 ` — note` 尾注或移入 Forbidden/Constraints。

[REQ-BOUNDARY-PATH-EXPRESSIONS-SINGLE-SOURCE] 边界模式的识别、规范化与匹配 MUST 只有一份实现，供 boundaries verifier、plan 扫描与 MCP `spec_allows_path` 共同调用。

## Scenarios

Rule: REQ-BOUNDARY-PATH-EXPRESSIONS-ALLOW-IS-PATH

Scenario: 裸根目录文件与任意扩展名被接受
  Given Allowed Changes 含 `Cargo.toml`、`` `CLAUDE.md` ``、`LICENSE`、`tools/x/gate.json`
  When 变更集包含这四个文件
  Then 边界层输出的四条 step verdict 均为 pass

Rule: REQ-BOUNDARY-PATH-EXPRESSIONS-NO-SILENT-DROP

Scenario: 无扩展名根文件不再被丢弃
  Given Allowed Changes 只含 `Makefile`
  When 收集允许模式
  Then 返回的允许模式列表包含 `Makefile`

Rule: REQ-BOUNDARY-PATH-EXPRESSIONS-NORMALIZE

Scenario: 反引号、反斜杠与前导点斜杠被规范化
  Given Allowed Changes 含 `` `./src\lib.rs` ``
  When 计算规范化模式
  Then 返回 `src/lib.rs`

Rule: REQ-BOUNDARY-PATH-EXPRESSIONS-NOTE

Scenario: 尾注被剥离而括号保留
  Given Allowed Changes 含 `` `Cargo.toml` — dev-dep only `` 与 `docs/foo (copy).md`
  When 计算规范化模式
  Then 返回的模式列表为 `Cargo.toml` 与 `docs/foo (copy).md`

Rule: REQ-BOUNDARY-PATH-EXPRESSIONS-PAREN-KEPT

Scenario: 括号是文件名的一部分
  Given Allowed Changes 含 `docs/foo (copy).md`
  When 变更集包含 `docs/foo (copy).md`
  Then 边界层输出的 step verdict 为 pass

Rule: REQ-BOUNDARY-PATH-EXPRESSIONS-DENY-TOKEN

Scenario: 自然语言禁令不成为路径模式
  Given Forbidden 含 "Do not modify `src/sliding_sync.rs`" 与 `src/sliding_sync.rs`
  When 收集禁止模式
  Then 返回的禁止模式列表只包含 `src/sliding_sync.rs`

Rule: REQ-BOUNDARY-PATH-EXPRESSIONS-NO-PROSE-EXTRACT

Scenario: 提到路径的禁令不误伤修改
  Given Forbidden 只含 "Do not break the JSON shape in `src/spec_report/mod.rs`"
  When 变更集包含 `src/spec_report/mod.rs`
  Then 边界层输出的 step verdict 为 pass

Rule: REQ-BOUNDARY-PATH-EXPRESSIONS-SHAPE-LINT

Scenario: 带说明的 Allow 条目被告警
  Given Allowed Changes 含 `` `Cargo.toml` (dev-dep only) ``
  When 运行 lint
  Then 输出 boundary-entry-shape Warning 并给出尾注写法

Rule: REQ-BOUNDARY-PATH-EXPRESSIONS-SINGLE-SOURCE

Scenario: 三处结论一致
  Given 一份 Allowed Changes 含 `./Cargo.toml` 的 spec 与变更 `Cargo.toml`
  When 分别经 verifier、plan 与 MCP spec_allows_path 判定
  Then 三处返回的判定结果均为 allowed

## Dependencies

None.

## Source Trace

- practice feedback: robrix2 docs/agent-spec-feedback-2026-08.md B1/B2（P0）
- triage: docs/robrix2-feedback-triage-2026-08.md 队列 1
- code: src/spec_verify/boundaries.rs looks_like_path_boundary；src/spec_gateway/plan.rs looks_like_path；src/spec_mcp/tools.rs spec_allows_path

## Open Questions

None.

## Next

Single exit: compile this requirement into a task contract with
`agent-spec requirements draft-specs`.
