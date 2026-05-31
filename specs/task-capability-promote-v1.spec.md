spec: task
name: "Capability 层 + promote v1：长寿命真相库"
inherits: project
tags: [bdd, capability, promote, phase3]
depends: [task-coverage-matrix-v1]
estimate: 3d
---

## 意图

引入 BDD-spine 的累积真相层:capability spec(`specs/capabilities/<name>.spec.md`)持有
长寿命的 Rule(系统当前行为的承诺),task spec 用 Example 证明它们。提供 `promote`:
当一条 task-scope Rule 的所有 Example 都机械通过后,把它提升进 capability spec(scope
从 Task 变为 Capability,id 不变)。这是 OpenSpec 活规格库的 BDD-native 形态:
promote 的前置门禁是真实跑过的测试,而不是文档完整性。

## 已定决策

- 新增 `SpecLevel::Capability`;`spec: capability` 解析为该级别。
- capability spec 的 `## 完成条件` 下的 `Rule:` 行,scope 解析为 `RuleScope::Capability(<spec-name>)`(不是 Task)。capability spec 的 Rule 通常没有 Example(Example 住在 task),因此 `bdd-rule-grouping` 的"空 Rule" warning 对 capability spec 不适用(降级为 info 或不触发)。
- task spec 新增可选 frontmatter `capability: <name>`(additive,`SpecMeta.capability: Option<String>`)。
- parser 按 spec 级别决定 Rule scope:Task 级 → `Task(file-stem)`,Capability 级 → `Capability(name)`。
- `BehaviorRule` 新增 additive `events: Vec<RuleEvent>`(provenance event log);`RuleEvent { kind: created|promoted|affirmed|deprecated, note }`。serde default skip-if-empty。
- 新命令 `agent-spec promote <task-spec> --rule <id> --to <capability-name> --code .`:
  - 门禁:对 task 跑验证,该 Rule 名下所有 Example 的 verdict 必须全为 `pass`,否则拒绝提升(非零退出)。
  - 合并:把该 Rule 以 `Capability(<name>)` scope 追加进 `specs/capabilities/<name>.spec.md`(不存在则创建一个 capability spec)。
  - 幂等:capability spec 已有同 id 的 Rule 时不重复追加。
  - 事件:被提升的 Rule 在 capability spec 中带一条 `promoted` 事件。
- observability/治理动作,**不改变 `is_passing` 语义**;promote 失败只阻止提升,不改 task 验证结果。

## 边界

### 允许修改

- src/spec_core/ast.rs（SpecLevel::Capability、RuleEvent/RuleEventKind、BehaviorRule.events、SpecMeta.capability —— 均 additive）
- src/spec_parser/meta.rs（解析 `spec: capability` + `capability:` 字段）
- src/spec_parser/parser.rs（按级别决定 Rule scope;capability 级 Rule 解析）
- src/spec_parser/resolver.rs（capability spec 解析支持）
- src/spec_lint/linters.rs（bdd-rule-grouping 对 capability 级不报"空 Rule" warning）
- src/spec_gateway/**
- src/spec_report/**
- src/main.rs（新增 `promote` 子命令 + 合并写回逻辑）
- README.md
- examples/**
- specs/capabilities/**（promote 产物目录）

### 禁止做

- 不要修改 `is_passing` / verdict 判定逻辑。
- 不要在本期实现 capability Rule 反向继承进 task 验证(读回),也不要做 capability 依赖图——留待 Phase 3.5/后续(见排除范围)。
- 不要让 promote 在门禁未通过时仍写回。
- additive 字段导致的构造点机械补全沿用既有 carve-out(仅补字段,不改逻辑)。

## 完成条件

### Rule: capability-spec-level — capability spec 是独立级别且 Rule 带 Capability scope

场景: spec capability 级别被解析
  测试:
    过滤: test_parse_capability_spec_level
  假设 一份 frontmatter 为 `spec: capability` 的 spec
  当 parser 解析
  那么 `meta.level` 为 `SpecLevel::Capability`

场景: capability spec 的 Rule 带 Capability scope
  测试:
    过滤: test_capability_spec_rule_has_capability_scope
  假设 一份名为 `ecosystem-import` 的 capability spec,`## 完成条件` 下有 `Rule: import-preserves-traceability`
  当 parser 解析
  那么 该 BehaviorRule 的 `key.scope` 为 `Capability("ecosystem-import")`

场景: 非法 spec 级别被拒绝
  测试:
    过滤: test_unknown_spec_level_rejected
  假设 frontmatter 为 `spec: nonsense`
  当 parser 解析
  那么 返回错误,指出未知 spec 级别

### Rule: task-declares-capability — task 可声明所属 capability

场景: task 的 capability frontmatter 字段被解析
  测试:
    过滤: test_parse_task_capability_field
  假设 一份 task spec frontmatter 含 `capability: ecosystem-import`
  当 parser 解析
  那么 `meta.capability` 为 `Some("ecosystem-import")`

场景: 无 capability 字段时为 None 且 JSON 不输出该键
  测试:
    过滤: test_task_without_capability_is_none_additive
  假设 一份不含 `capability:` 的旧 task spec
  当 parser 解析并序列化为 JSON
  那么 `meta.capability` 为 `None`
  并且 JSON 不出现 `capability` 键

### Rule: promote-gates-on-passing-examples — 提升前所有 Example 必须机械通过

场景: 所有 Example 通过时 Rule 被提升进 capability spec
  测试:
    过滤: test_promote_appends_rule_when_examples_pass
  假设 一份 task spec 的 Rule `r-ok` 名下所有 Example 的 verdict 为 `pass`
  当 运行 `agent-spec promote <task> --rule r-ok --to billing --code .`
  那么 `specs/capabilities/billing.spec.md` 中出现 id 为 `r-ok`、scope 为 Capability 的 Rule
  并且 该 Rule 带一条 `promoted` 事件

场景: 有 Example 未通过时拒绝提升
  测试:
    过滤: test_promote_refuses_when_an_example_fails
  假设 task spec 的 Rule `r-bad` 名下有 Example 的 verdict 不是 `pass`
  当 运行 `agent-spec promote <task> --rule r-bad --to billing --code .`
  那么 命令以非零状态失败,提示门禁未通过
  并且 `specs/capabilities/billing.spec.md` 不包含 `r-bad`

场景: 重复提升同一 Rule 幂等
  测试:
    过滤: test_promote_is_idempotent_for_same_rule
  假设 capability spec `billing` 已含 id 为 `r-ok` 的 Rule
  当 再次提升 `r-ok` 到 `billing`
  那么 capability spec 中 `r-ok` 只出现一次

场景: 提升不存在的 Rule id 报错
  测试:
    过滤: test_promote_unknown_rule_id_errors
  假设 task spec 中没有 id 为 `r-missing` 的 Rule
  当 运行 promote `--rule r-missing`
  那么 命令失败,提示找不到该 Rule id

### Rule: rule-event-log-is-additive — Rule 事件日志只增不减

场景: 新建 Rule 默认无事件且 JSON 不输出该键
  测试:
    过滤: test_rule_events_additive_empty_by_default
  假设 一份普通 task spec 的 Rule
  当 序列化为 JSON
  那么 不出现 `events` 键

场景: 事件日志可序列化与反序列化
  测试:
    过滤: test_rule_event_roundtrips
  假设 一个带 `promoted` 事件的 `RuleEvent`
  当 序列化再反序列化
  那么 结果与原值相等

### Rule: promote-does-not-change-verdict-semantics — promote 是治理动作不改门禁

场景: promote 不改变 is_passing
  测试:
    过滤: test_promote_does_not_change_is_passing
  假设 一份全部场景 pass 的验证报告
  当 执行 promote 的合并逻辑后再调用 `is_passing`
  那么 `is_passing` 仍为 true
  并且 summary 计数未变

## 排除范围

- capability Rule 反向继承进 task 验证(读回 capability 真相到 task resolution)—— Phase 3.5
- capability 依赖图与 promote 跨能力前置校验 —— 后续
- `affirmed` / `deprecated` 事件的命令入口(本期只产出 `created` / `promoted`)
- capability spec 的多层继承(org → capability → task 的完整链)
- 把 capability 覆盖纳入 `is_passing` 或 guard 门禁
