spec: task
name: "Lint-ack + 五维分类 v1"
inherits: project
tags: [bdd, lint, governance, phase5]
depends: [task-discovery-questions-v1]
estimate: 2d
---

## 意图

兑现 Phase 1 预留的 `lint-ack` 机制:作者可以用 `<!-- lint-ack: <code> — <reason> -->`
显式确认(豁免)某条 lint 规则,被确认的诊断从主诊断列表移到"已确认"列表,带可审计理由。
同时把 lint 规则归入五个维度(Completeness / Clarity / Consistency / Coverage / Boundary),
让报告按维度聚合。这是 BurntSushi 式"从 AI 创建的例外开始 review"的落地。

## 已定决策

- `SpecDocument` 新增 additive 字段 `lint_acks: Vec<LintAck>`;parser 从 body 任意行扫描 `<!-- lint-ack: <code> — <reason> -->`(分隔符 em-dash 或 `:`)。
- `LintReport` 新增 additive 字段 `acknowledged: Vec<LintDiagnostic>`:pipeline 运行后,凡 `rule` 命中某个 ack 的 `code` 的诊断,从 `diagnostics` 移入 `acknowledged`(按规则码粗粒度确认)。
- ack 只能降噪,**不能把本应是 Error 的诊断变成通过**:Error 级诊断即使被 ack 也保留在 `diagnostics`(ack 仅适用于 Warning/Info)。
- 新增 `lint --format json` 输出 `acknowledged` 列表;`explain` markdown 增加"Acknowledged"小节(review 入口)。
- 五维分类:纯函数 `dimension_of(rule_code) -> Dimension`,把已有规则码映射到五维之一;`LintReport` 提供按维度聚合的计数(报告层,不改诊断本身)。
- 不改 `is_passing`;不改各 lint 的 severity 判定。

## 边界

### 允许修改

- src/spec_core/ast.rs（SpecDocument.lint_acks + LintAck,additive）
- src/spec_core/lint.rs（LintReport.acknowledged + Dimension + dimension_of,additive）
- src/spec_parser/parser.rs（扫描 lint-ack 标记）
- src/spec_lint/pipeline.rs（运行后按 ack 过滤 + 维度聚合）
- src/spec_report/**、src/main.rs（lint/explain 输出 acknowledged + 维度)
- README.md、examples/**、skills/agent-spec-authoring/**

### 禁止做

- 不要让 ack 抑制 Error 级诊断(Error 即使被 ack 也保留)。
- 不要改 `is_passing` / lifecycle 门禁。
- 本期不做 `--inferential-policy`、project.spec 冲突升 critical、跨 spec cross-check 提升为命令——见排除范围。
- additive 字段构造点机械补全沿用既有 carve-out。

## 完成条件

### Rule: lint-ack-suppresses-warnings — ack 把命中规则的 warning 移入 acknowledged

场景: 被 ack 的 warning 移出主诊断
  测试:
    过滤: test_lint_ack_moves_warning_to_acknowledged
  假设 一份 spec 触发 `open-question` warning,且 body 含 `<!-- lint-ack: open-question — 原型阶段不需要 -->`
  当 运行 lint pipeline
  那么 `diagnostics` 不再含 `open-question`
  并且 `acknowledged` 含该 `open-question` 诊断

场景: 未被 ack 的诊断不受影响
  测试:
    过滤: test_lint_ack_leaves_other_diagnostics
  假设 一份 spec 同时触发 `open-question` 与另一类 warning,只 ack 了 `open-question`
  当 运行 lint pipeline
  那么 另一类 warning 仍在 `diagnostics` 中

场景: ack 不能抑制 Error 级诊断
  测试:
    过滤: test_lint_ack_cannot_suppress_error
  假设 一个 Error 级诊断的规则码被 ack
  当 pipeline 应用 ack 过滤
  那么 该 Error 诊断仍保留在 `diagnostics`

### Rule: lint-ack-parsing — 标记被正确解析

场景: 解析 lint-ack 标记
  测试:
    过滤: test_parse_lint_ack_marker
  假设 一份 spec body 含 `<!-- lint-ack: bdd-rule-id — 故意留作示例 -->`
  当 parser 解析
  那么 `doc.lint_acks` 含一个 code 为 `bdd-rule-id`、reason 含"故意留作示例"的项

场景: 无 lint-ack 的旧 spec 字段为空且 JSON 不输出
  测试:
    过滤: test_lint_acks_additive_empty
  假设 当前任一不含 lint-ack 标记的旧 spec
  当 解析并序列化为 JSON
  那么 `lint_acks` 为空
  并且 JSON 不出现 `lint_acks` 键

### Rule: five-dimension-classification — 规则码归入五维

场景: 已知规则码映射到正确维度
  测试:
    过滤: test_dimension_of_known_rules
  假设 规则码 `coverage`、`bdd-scenario-shape`、`cross-check-boundary`
  当 调用 `dimension_of`
  那么 分别归入 Coverage、Clarity、Boundary 维度

场景: 未知规则码有兜底维度
  测试:
    过滤: test_dimension_of_unknown_falls_back
  假设 一个未登记的规则码
  当 调用 `dimension_of`
  那么 返回兜底维度(Consistency)而不 panic

### Rule: ack-is-additive-observability — ack 不改门禁

场景: ack 不改变 lint 通过性
  测试:
    过滤: test_ack_does_not_change_gating
  假设 一份仅含被 ack 的 warning 的 spec
  当 运行 lint
  那么 lint 通过性与 ack 前一致(warning 本就不 gate)
  并且 `acknowledged` 列表非空

## 排除范围

- `--inferential-policy`(trust/review/strict 对 inferential verdict 的门禁)—— Phase 5.5
- project.spec 冲突升为 critical severity + guard hard-fail —— Phase 5.5
- 跨 spec cross-check 提升为一等命令(`cross_check` 函数已存在)—— Phase 5.5
- 按 rule+line 的细粒度 ack(本期按规则码粗粒度)
- ack 标记的过期/失效检测
