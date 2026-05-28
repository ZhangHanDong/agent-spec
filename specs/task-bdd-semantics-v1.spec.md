spec: task
name: "BDD 语义增强 v1：Rule grouping + Scenario shape lint"
inherits: project
tags: [bdd, formulation, phase1]
estimate: 2d
---

## 意图

引入 BDD/Cucumber 原始语义中最关键、agent-spec 当前缺失的 Formulation 基元——
**Rule → Scenario** 的行为归属关系。让 task contract 能表达"规则由例子证明"，
并通过新增 lint 防止 scenario 退化成无规则、过程式、不可审查的脚本。

本期是 BDD-spine 吸纳路线的 Phase 1，只做 Formulation 层。它的存在价值是为
后续 Phase 2（机械覆盖矩阵）、Phase 3（capability 层与 promote）、Phase 4（Discovery
结构化产物）提供同一个 Rule 基元，避免日后回头改身份模型造成数据迁移。

## 已定决策

### Rule 身份模型（必须在 v1 定下来）

- `RuleKey = { scope, id }`：`id` 是稳定标识符，`name` 是可任意修改的人类显示文本，二者解耦。
- `id` **必须**是显式 kebab-case 标识符，正则 `^[a-z][a-z0-9-]*$`。`name` 为可选自由文本。
- DSL 语法（**必须**显式 id）：
  - `Rule: <id>` — id 同时作为 name
  - `Rule: <id> — <display name>` — id 与 name 分离（分隔符为 em dash `—` 或两个空格 `  `）
  - 中文等价：`规则: <id>` / `规则: <id> — <显示名>`
- **不做自动 slugify**。如果 Rule 行第一个 token 不是合法 kebab-case id（例如 `Rule: VIP 折扣优先于促销叠加`），parser **不**生成 BehaviorRule，lint 输出 `bdd-rule-id` warning，后续 scenario 不归属——这避免显示文本变更影响身份，也避免中英文 slug 算法歧义。
- `RuleScope` 是 Rust 枚举 `{ Task(String), Capability(String), Project }`：
  - Task scope 携带 **task spec 文件 stem**（去掉 `.spec` / `.spec.md` 后缀）作为 namespace，**不**用人类显示名 `meta.name`。文件 stem 是稳定文件级标识，未来 spec_id 字段就位时可平滑替换。
  - Capability / Project 作为 reserved 变体写入 AST，**v1 不解析、不加载、不提升**。
- 当 task spec 写 `Rule: foo` 时，scope 自动填 `Task("<file-stem>")`。

### Example == Scenario

- Cucumber 文档把 Example 与 Scenario 视为同义；v1 沿用此约定。
- DSL 支持 `Example:` / `例子:` / `示例:` 作为 `Scenario:` / `场景:` 的别名。
- **AST 不引入新的 Example 节点**；解析后统一存为 `Scenario`。verification 路径不动。

### 兼容性承诺

- 本期**不改变现有 scenario / test / boundary 的验证通过语义**。
- 新增 AST 字段全部 additive、`#[serde(default, skip_serializing_if = ...)]`。
- 任何现有 `.spec` / `.spec.md` 不改动即可继续 parse、lint、verify、lifecycle 通过。
- 新增 lint 默认 warning/info 级，**不进入 `is_passing` 的决策路径**。
- "lint-only" 不是本期承诺；"verification semantics 不变 + AST/JSON additive + 新 lint 不入 verdict" 才是。

### Lint 诊断格式（agent-readable self-correction guidance）

v1 新增的所有 lint 诊断消息必须采用**自我纠正指南**格式, 写给 AI agent 自我纠正读, 不是给 human reviewer 读(human 看的是 explain 输出):

- (1) **为什么这条规则存在**: 短句说明规则解决的工程问题(避免 id 漂移、避免 scenario 退化为脚本、避免 Rule 引用断裂等)。
- (2) **如何修复**: 给出具体的 before / after 语法示例。
- (3) **何时可以不修**: 如果有合理例外, 明示例外路径(例如 throwaway prototype 不需要 Rule 分组)。
- (4) **不修时未来如何留痕**: forward-reference 到 Phase 5 将实现的 `<!-- lint-ack: <code> — <reason> -->` 标记。**v1 只在诊断文案中预留这个语法**, 不实现完整的 acknowledged_warnings 字段、explain 渲染、过期策略——那是 Phase 5 severity 治理 phase 的工作。

这件事不改 lint 引擎、不改 parser、不改 AST, 只是写作纪律: 所有 v1 新增 lint 的 `suggestion` 字段都要含这四要素。

### v1 不做（明确划出）

- 不解析、不加载 capability spec 文件，不实现 capability inheritance。
- 不实现 promote / archive / capability-level Rule 合并语义。
- 不引入 `## Questions` / `<!-- NEEDS CLARIFICATION -->` 顶层节——会撞当前 parser 与 authoring skill 的"未知顶层 section 报错"规则，留给 Phase 4 同步改 `Section` enum / parser / lint / skills / AGENTS。
- 不实现 Gherkin `Background` / `Scenario Outline` / `Examples` 表驱动展开。
- 不实现 `.feature` 文件互操作。
- 不修改 `src/spec_verify/test_verifier.rs`、`boundaries.rs`、`ai_verifier.rs`。

## 边界

### 允许修改

- src/spec_core/ast.rs
- src/spec_parser/keywords.rs
- src/spec_parser/parser.rs
- src/spec_gateway/brief.rs
- src/spec_gateway/plan.rs
- src/spec_gateway/lifecycle.rs（**仅限**在已有 `#[cfg(test)] mod tests` 内追加回归测试函数）
- src/spec_lint/linters.rs
- src/spec_lint/pipeline.rs
- src/spec_report/**
- src/main.rs（**仅限**在已有 `#[cfg(test)] mod tests` 内追加 CLI / JSON 输出端到端测试）
- README.md
- skills/agent-spec-authoring/**
- examples/**

### 禁止做

- 不要修改 docs/comparison-openspec-speckit.md（独立任务）。
- 不要改动 src/spec_verify/test_verifier.rs、src/spec_verify/boundaries.rs、src/spec_verify/ai_verifier.rs、src/spec_verify/mod.rs 中的**验证逻辑**——verification 通过语义不能动。**例外（实施中发现、机械必需）**：新增 `Scenario.rule` / `Section::AcceptanceCriteria.rules` 是 additive 字段，Rust 要求所有 struct 字面量补全字段；因此允许**仅在这些文件已有的 `#[cfg(test)]` 测试夹具里**机械补 `rule: None` / `rules: vec![]`，不得改动任何非测试代码或验证语义。此偏离记入 Phase 1 retrospective。
- 不要把 Rule / 新 lint 写进 `is_passing` 的决策路径。
- 不要修改 `is_passing` / `is_passing_with_review_mode` 函数体；不要改 lifecycle / guard 主路径逻辑；不要改 CLI parser 主路径或任何现有 public API。`lifecycle.rs` 与 `main.rs` 的允许修改**仅限**在已有 `#[cfg(test)] mod tests` 内**追加**新测试函数。
- 不要在 v1 引入 capability spec 文件类型、加载逻辑或新顶层 section。
- 不要让任何新 lint 默认升级为 error；新 lint 全部 warning/info。
- 不要让 `bdd-implementation-detail-step` 关键词列表硬编码进 verdict，只能产出 info。

## 完成条件

### Rule 与 Parser 行为

场景: Rule 头部解析为 BehaviorRule
  测试:
    过滤: test_parse_rule_header_creates_behavior_rule
  假设 一份 task spec（文件名 `<file-stem>.spec.md`）在 `## 完成条件` 下出现 `Rule: auth-must-not-leak — 鉴权失败不得泄漏内部错误`
  当 parser 解析该 spec
  那么 AST 中出现 `BehaviorRule { key: RuleKey { scope: Task("<file-stem>"), id: "auth-must-not-leak" }, name: "鉴权失败不得泄漏内部错误" }`
  并且 该 Rule 之后的 scenarios 的 `rule` 字段为 `Some("auth-must-not-leak")`

场景: Rule 行只有 id 时 name 退回为 id
  测试:
    过滤: test_parse_rule_header_without_display_name
  假设 spec 中出现 `Rule: refund-must-be-idempotent`
  当 parser 解析
  那么 BehaviorRule 的 `id` 为 `refund-must-be-idempotent`
  并且 `name` 等于 `id`

场景: Rule 行缺少显式 kebab-case id 时触发 warning 且不归属
  测试:
    过滤: test_freeform_rule_emits_warning_and_does_not_group_scenarios
  假设 spec 中出现 `Rule: VIP 折扣优先于促销叠加`（首 token 不匹配 `^[a-z][a-z0-9-]*$`）
  当 parser 解析
  那么 AST 中**不**生成对应的 BehaviorRule（不做自动 slugify）
  并且 该 Rule 行后的 scenarios 的 `rule` 字段为 `None`
  并且 lint 输出 `bdd-rule-id` warning，提示需要显式 kebab-case id

场景: 中文规则别名
  测试:
    过滤: test_parse_chinese_rule_alias
  假设 spec 中出现 `规则: vip-discount-priority — VIP 折扣优先级高于促销`
  当 parser 解析
  那么 解析结果等价于英文 `Rule:` 写法
  并且 后续中文 `场景:` 或 `示例:` 均归属到该 Rule

场景: Example / 示例 / 例子 别名等价于 Scenario
  测试:
    过滤: test_parse_example_alias_as_scenario
  假设 spec 中存在 `Example: 余额充足时提现成功` 与 `示例: 余额不足时提现失败`
  当 parser 解析
  那么 AST 中两条都是 `Scenario` 节点
  并且 AST 中不存在新的 `Example` 节点类型

场景: 无 Rule 的旧 spec 兼容（verdict 与 contract/plan 格式不变；lint 可有非阻塞 info）
  测试:
    过滤: test_legacy_spec_without_rule_compat
  假设 当前仓库 `examples/` 与 `specs/` 下任一不含 Rule 的旧 spec
  当 parser 解析
  那么 所有 scenario 的 `rule` 字段为 `None`
  并且 `contract` 与 `plan` 对无 Rule spec 保持当前扁平 `Scenario:` 列表格式
  并且 `lifecycle` / `guard` 的 verdict 与 summary 计数（total / passed / failed / skipped / uncertain）与 v1 实现前一致
  并且 当 lint 因 v1 新规则输出 `bdd-rule-grouping` info 时，lint 整体 verdict 仍不变为 fail

### AST / JSON

场景: BehaviorRule 与 RuleScope 在 JSON 中序列化
  测试:
    过滤: test_rule_scope_serializes_to_json
  假设 一份带 Rule 的 task spec（文件名 `<file-stem>.spec.md`）
  当 通过 `--format json` 输出 contract 或 plan
  那么 JSON 含 `rules: [{ key: { scope: { task: "<file-stem>" }, id: "<id>" }, name: "<name>", scenario_names: [...] }]`
  并且 每个被归属的 scenario JSON 含 `rule: "<id>"`

场景: JSON 输出只增不减
  测试:
    过滤: test_json_output_additive_only
  假设 一个 v0.2.7 schema 的旧消费方
  当 v1 输出 lifecycle JSON
  那么 旧字段名 / 类型 / 必填性全部不变
  并且 新字段 `rules` / scenario `rule` 全部 `#[serde(default, skip_serializing_if = ...)]`

场景: Capability scope 是 reserved 类型
  测试:
    过滤: test_capability_scope_is_reserved_in_v1
  假设 `RuleScope::Capability(name)` 与 `RuleScope::Project` 在 ast.rs 中已声明
  当 v1 parser 解析任何 DSL 文本
  那么 不产生 Capability 或 Project scope 的 BehaviorRule
  并且 序列化、反序列化 Capability/Project scope 的 AST 值不报错

### 新 Lint（warning/info，不入 verdict）

场景: bdd-rule-grouping 对无 Rule 多场景提示
  测试:
    过滤: test_bdd_rule_grouping_suggests_when_three_or_more_scenarios_uncategorized
  假设 一份 task spec 在 `## 完成条件` 下有 3 个及以上 scenario 且没有任何 Rule
  当 运行 `agent-spec lint`
  那么 输出含 `bdd-rule-grouping` info，建议引入 Rule 分组
  并且 lint 整体 verdict 不变为 fail

场景: bdd-rule-grouping 警告空 Rule
  测试:
    过滤: test_bdd_rule_grouping_warns_on_empty_rule
  假设 spec 中存在一个 Rule 但其下没有任何 scenario
  当 运行 lint
  那么 输出含 `bdd-rule-grouping` warning，指明空 Rule 的 id

场景: bdd-scenario-shape 检查缺失 When / Then
  测试:
    过滤: test_bdd_scenario_shape_flags_missing_when_or_then
  假设 spec 中存在一个 scenario 缺少 `When/当` 或缺少 `Then/那么`
  当 运行 lint
  那么 输出含 `bdd-scenario-shape` warning

场景: bdd-scenario-shape 警告首步为 And / But / 并且 / 但是
  测试:
    过滤: test_bdd_scenario_shape_flags_leading_and_or_but
  假设 一个 scenario 的第一个 step 关键字是 `And` / `But` / `并且` / `但是` 任一
  当 运行 lint
  那么 输出含 `bdd-scenario-shape` warning

场景: bdd-implementation-detail-step 识别中英文过程式动词
  测试:
    过滤: test_bdd_implementation_detail_flags_ui_verbs_en_and_zh
  假设 一个 scenario 包含 `click` / `type` / `visit` / `data-testid` / CSS selector 任一英文关键词
  当 运行 lint
  那么 输出含 `bdd-implementation-detail-step` info
  并且 中文 `点击` / `输入` / `访问` / `填写` / `选择` 任一也触发同一条 info

场景: 新 lint 不影响 lifecycle verdict
  测试:
    过滤: test_new_bdd_lints_do_not_affect_lifecycle_verdict
  假设 一份 spec 同时触发 `bdd-rule-grouping` warning 与 `bdd-implementation-detail-step` info 但所有绑定测试通过
  当 运行 `agent-spec lifecycle <spec> --code .`
  那么 `is_passing` 仍为 true
  并且 `report.summary.failed` / `skipped` / `uncertain` 全为 0

场景: 新增 lint 诊断包含自我纠正指南四要素
  测试:
    过滤: test_new_bdd_lints_emit_self_correction_guidance
  假设 一份 spec 触发 `bdd-rule-id` / `bdd-rule-grouping` / `bdd-scenario-shape` / `bdd-implementation-detail-step` 任一 lint
  当 运行 `agent-spec lint --format json`
  那么 该诊断的 `suggestion` 字段含"为什么这条规则存在"的短句
  并且 `suggestion` 字段给出具体的 before / after 语法示例
  并且 `suggestion` 字段说明合理例外的路径
  并且 `suggestion` 字段提及未来通过 `<!-- lint-ack: <code> — <reason> -->` 留痕的 forward-reference

### Contract / Plan 渲染

场景: contract 按 Rule 分组输出
  测试:
    过滤: test_contract_renders_scenarios_grouped_by_rule
  假设 spec 含 2 个 Rule，每个 Rule 下 2 个 scenario
  当 运行 `agent-spec contract <spec>`
  那么 默认文本输出按 Rule 分组，每个 Rule 标题下列出归属 scenario
  并且 无 Rule 的旧 spec 输出保持当前扁平 `Scenario:` 列表格式

场景: plan --format prompt 在 Task Sketch 段包含 Rule 分组
  测试:
    过滤: test_plan_prompt_includes_rule_grouping
  假设 spec 含 Rule 与若干 scenario
  当 运行 `agent-spec plan <spec> --code . --format prompt`
  那么 Task Sketch 段按 Rule 分组输出
  并且 agent 阅读时能看到"哪些例子证明哪条规则"

### Verification 通过语义不变（回归保护）

场景: 现有 spec 在 v1 后的 guard verdict 完全不变（lint 输出可有新增 info）
  测试:
    过滤: test_existing_specs_pass_lifecycle_after_v1_changes
  假设 仓库现有 `specs/*.spec.md` 与 `examples/*.spec` 全集
  当 在 v1 实现完成后运行 `agent-spec guard --spec-dir specs --code .`
  那么 每份 spec 的 verdict 与 summary 计数（passed / failed / skipped / uncertain）与 v1 实现前一致
  并且 `is_passing` 公式 `total > 0 && failed == 0 && skipped == 0 && uncertain == 0` 未被新代码路径改写
  并且 新增的 lint info / warning 不改变 guard 的通过 / 失败结果

## 排除范围

- Capability spec 文件加载、引用、提升语义（Phase 3）
- `## Questions` / `<!-- NEEDS CLARIFICATION -->` 顶层节与 Discovery 工作流（Phase 4）
- 机械覆盖矩阵 Rule × Scenario × Test × Verdict 报告（Phase 2，独立任务）
- Gherkin `Background` / `Scenario Outline` / `Examples` 表驱动展开
- `.feature` 文件互操作
- AI verifier、caller mode、boundary verifier、test verifier 任何变动
- README 之外的市场叙事文档变动（comparison 文档由独立任务负责）
