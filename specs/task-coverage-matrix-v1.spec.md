spec: task
name: "机械覆盖矩阵 v1：Rule × Scenario × Test × Verdict"
inherits: project
tags: [bdd, coverage-matrix, phase2]
depends: [task-bdd-semantics-v1]
estimate: 2d
---

## 意图

把 agent-spec 现有的显式 `Test:` selector 变现成一张**机械组装**的覆盖矩阵:
每个 scenario 一行,列出它归属的 Rule、绑定的测试选择器、该测试是否真实存在、
验证 verdict,以及该 verdict 来自机械验证还是 AI 推理(provenance)。
矩阵是 BDD-spine 路线 Phase 2,作为 Phase 3(capability)与 Phase 4(discovery)的
共同数据底座。它顺带提供 §9.4 承诺的"dangling selector 检测"——指向不存在测试的
selector 被机械标出,缓解 `Test:` selector 与测试函数名耦合的代价。

## 已定决策

- 新增只读子命令 `agent-spec matrix <spec> --code .`,支持 `--format text|json|markdown`。
- **CLI 运行语义锁定**:`matrix` 内部跑一次验证,语义等价于 `verify` 的默认模式;接受与 `verify` 同款 flags:`--ai-mode`(默认 `off`)、`--change`、`--change-scope`(默认 `none`)。verdict 列直接来自这次验证报告,不另搞一套验证语义,避免与 `verify`/`lifecycle` 分叉。
- 矩阵核心是纯函数 `build_coverage_matrix(resolved, report, test_index) -> CoverageMatrix`,不读全局状态、不调用 LLM;`test_index` 由调用方预先机械扫描得到。
- 每行字段:`rule`(归属 Rule id,无则 `—`)、`scenario`、`test_selector`(无则 `—`)、`test_found`(`found` / `missing` / `none`)、`verdict`、`provenance`(`computational` / `inferential` / `—`)。
- **`test_found` 定义为精确测试函数名存在性检查**(不是 cargo runner 的 filter 语义):机械扫描 `--code` 路径下 Rust 源中的 `#[test]` / `#[tokio::test] fn <name>`,建索引;`selector.filter` **精确等于**某个被收集的函数名记 `found`,无 selector 记 `none`,其余(包括指向不存在函数、或写成 substring/模块路径而非精确函数名)记 `missing`。v1 策略:`Test:` filter 应是精确 Rust 测试函数名。此口径与 `cargo test -q <filter>` 的子串/模块匹配语义**有意不同**——matrix 检查"这个 selector 指向一个真实存在的测试函数吗",dangling 必须被显式标出而非被 cargo 的宽松匹配掩盖。跨 crate 的 package 限定精确解析是 v1 已知限制(见排除范围)。
- **`provenance` 双重打戳 + 派生兜底**,保证 caller-mode 也进统一通道:
  - `ScenarioResult` 上新增 additive 字段 `Option<EvidenceProvenance>`(`Computational` / `Inferential`)。
  - `run_verification` 按产出该结果的 verifier 名打戳:`ai` → `Inferential`,其余(`test`/`boundaries`/`structural`/`complexity`)→ `Computational`,未覆盖(skip)→ `None`。各 verifier 自身不改 verdict 逻辑。
  - `resolve-ai` 把 Skip 结果改写成 AI decision 时,**同步置 `provenance = Inferential`**(它绕过 AiVerifier,必须自己打戳)。
  - matrix 作为兜底:若某结果 `provenance` 为 `None` 但 `evidence` 含 `Evidence::AiAnalysis`,矩阵按 `Inferential` 显示。
- `explain --format markdown` 内嵌该矩阵(作为新段落,PR 验收材料)。
- 矩阵是 observability,**不改变 `is_passing` 语义,不引入任何新门禁**。

## 边界

### 允许修改

- src/spec_core/verify.rs（**仅新增** `EvidenceProvenance` 类型 + `ScenarioResult.provenance` additive 字段；不改 verdict/汇总计算逻辑）
- src/spec_verify/mod.rs（**仅在** `run_verification` 按 verifier 名打 provenance 戳 + 构造点补字段；不改各 verifier 的 verdict）
- src/spec_verify/structural.rs、src/spec_verify/complexity.rs、src/spec_verify/boundaries.rs、src/spec_verify/ai_verifier.rs、src/spec_verify/test_verifier.rs（**仅** 在各自 `ScenarioResult` 字面量机械补 `provenance: None` 字段初始化器,因 additive 字段不可避免;**不改任何 verdict 判定逻辑**)
- src/spec_report/**
- src/spec_gateway/**
- src/main.rs（新增 `matrix` 子命令 + explain markdown 内嵌矩阵 + `resolve-ai` 写回 AI decision 时置 `provenance = Inferential`）
- README.md
- examples/**

> 注:边界检查的 verdict 仍是 `BoundariesVerifier` 产出的**单条 synthetic scenario**(`[boundaries] ...`),作为普通 row 出现在矩阵里,**不**作为每个业务 scenario 的 per-row 列。v1 不引入 `boundary_relevant` 列。

### 禁止做

- 不要修改 `is_passing` / `is_passing_with_review_mode` 函数体,或任何 verifier 的 verdict 判定逻辑。
- 不要让覆盖矩阵的组装过程调用 AI / LLM——矩阵结构必须机械可复现。
- 不要把覆盖率写进 lifecycle/guard 的通过失败决策(本期不加门禁)。
- 不要在本期实现 capability-scope 行(Phase 3)或跨 spec cross-check(Phase 5)。
- `verify.rs` / `mod.rs` 的改动仅限上面"允许修改"括注的范围;新增 additive 字段导致的测试夹具机械补全沿用 Phase 1 的 carve-out(仅 `#[cfg(test)]` 夹具补 `provenance: None`)。

## 完成条件

### Rule: matrix-assembly-is-mechanical — 矩阵由 parser + 测试扫描 + 验证报告机械组装

场景: 每个 scenario 一行且字段正确
  测试:
    过滤: test_matrix_has_one_row_per_scenario
  假设 一份 spec 含 1 个 Rule、2 个绑定到存在测试的 scenario
  当 调用 `build_coverage_matrix` 并全部验证通过
  那么 矩阵恰好有 2 行
  并且 每行的 `rule` 为该 Rule id、`test_selector` 为对应 selector、`verdict` 为 `pass`

场景: markdown 格式渲染为表格
  测试:
    过滤: test_matrix_markdown_renders_table
  假设 一份含 Rule 与 scenario 的 spec
  当 运行 `agent-spec matrix <spec> --code . --format markdown`
  那么 输出包含一个 markdown 表格,表头含 `Rule`、`Scenario`、`Test`、`Verdict`、`Provenance` 列

场景: json 格式可机器解析
  测试:
    过滤: test_matrix_json_is_machine_parseable
  假设 一份含 Rule 与 scenario 的 spec
  当 运行 `agent-spec matrix <spec> --code . --format json`
  那么 输出是合法 JSON
  并且 含 `rows` 数组,每个元素有 `scenario`、`test_found`、`verdict` 字段

### Rule: dangling-selector-detection — 机械标出指向不存在测试的 selector

场景: selector 指向不存在的测试记为 missing
  测试:
    过滤: test_matrix_flags_dangling_selector_as_missing
  假设 某 scenario 绑定 `测试: test_does_not_exist_anywhere`,且代码中无此测试函数
  当 构建覆盖矩阵
  那么 该行 `test_found` 为 `missing`
  并且 矩阵整体不会因此 panic 或中止

场景: test_found 要求精确函数名而非子串匹配
  测试:
    过滤: test_matrix_test_found_requires_exact_function_name
  假设 测试索引含函数 `test_register_returns_201`,某 scenario 绑定 `测试: register`(子串,非精确函数名)
  当 构建覆盖矩阵
  那么 该行 `test_found` 为 `missing`
  并且 精确绑定 `测试: test_register_returns_201` 的 scenario 同图中记 `found`

场景: 无 selector 的 scenario 记为 none
  测试:
    过滤: test_matrix_marks_scenario_without_selector_as_none
  假设 某 scenario 没有 `测试:` selector
  当 构建覆盖矩阵
  那么 该行 `test_selector` 为 `—`、`test_found` 为 `none`
  并且 该行 `verdict` 为 `skip`

### Rule: provenance-distinguishes-evidence-source — verdict 标注机械还是推理来源

场景: 机械 verifier 的 verdict 标为 computational
  测试:
    过滤: test_provenance_test_verifier_is_computational
  假设 某 scenario 由 TestVerifier 产出 `pass`
  当 `run_verification` 汇总结果
  那么 该 `ScenarioResult.provenance` 为 `Computational`

场景: AI stub 的 uncertain 标为 inferential
  测试:
    过滤: test_provenance_ai_stub_is_inferential
  假设 某未被机械 verifier 覆盖的 scenario 在 `AiMode::Stub` 下产出 `uncertain`
  当 `run_verification` 汇总结果
  那么 该 `ScenarioResult.provenance` 为 `Inferential`

场景: caller-mode 经 resolve-ai 写回的结果标为 inferential
  测试:
    过滤: test_provenance_resolve_ai_is_inferential
  假设 一个 Skip 结果经 `resolve-ai` 用外部 AI decision 改写
  当 写回 AI decision 时
  那么 该结果 `provenance` 为 `Inferential`
  并且 该结果 `evidence` 含 `Evidence::AiAnalysis`

场景: 矩阵从 AiAnalysis 证据兜底派生 inferential
  测试:
    过滤: test_matrix_derives_inferential_from_ai_evidence
  假设 某结果 `provenance` 为 `None` 但 `evidence` 含 `Evidence::AiAnalysis`
  当 构建覆盖矩阵
  那么 该行 `provenance` 显示为 `inferential`

### Rule: matrix-is-observability-not-a-gate — 矩阵不改变验证语义

场景: 构建矩阵不改变 is_passing
  测试:
    过滤: test_matrix_does_not_change_is_passing
  假设 一份全部场景 verdict 为 `pass` 的验证报告
  当 构建并渲染覆盖矩阵后再调用 `is_passing`
  那么 `is_passing` 仍为 true
  并且 `report.summary` 的各计数未被矩阵代码改动

场景: provenance 字段 JSON 只增不减
  测试:
    过滤: test_json_provenance_additive_only
  假设 一个未打 provenance 的 `ScenarioResult`(provenance 为 None)
  当 序列化为 JSON
  那么 不出现 `provenance` 键
  并且 旧消费方看到的结构不变

场景: 未分组 scenario 的 rule 列为占位符
  测试:
    过滤: test_matrix_ungrouped_scenario_rule_column_is_dash
  假设 某 spec 的 scenario 没有归属任何 Rule
  当 构建覆盖矩阵
  那么 该行 `rule` 为 `—`
  并且 矩阵正常生成不报错

### Rule: matrix-command-matches-verify-semantics — matrix 运行语义对齐 verify

场景: matrix 默认以 verify 默认模式运行
  测试:
    过滤: test_matrix_command_runs_verification_in_default_mode
  假设 一份 scenario 未被机械 verifier 覆盖的 spec
  当 运行 `agent-spec matrix <spec> --code .`(不带 `--ai-mode`)
  那么 该行 `verdict` 为 `skip`(默认 `--ai-mode off`,与 `verify` 默认一致)
  并且 不产出 `uncertain`(未隐式开启 AI)

### Rule: explain-embeds-the-matrix — 覆盖矩阵进入 PR 验收材料

场景: explain markdown 内嵌覆盖矩阵
  测试:
    过滤: test_explain_markdown_embeds_coverage_matrix
  假设 一份含 Rule、scenario 与绑定测试的 spec
  当 运行 `agent-spec explain <spec> --code . --format markdown`
  那么 输出包含覆盖矩阵表格段落
  并且 同时保留原有的 Contract 与验证结果摘要

## 排除范围

- capability-scope 行与 promote(Phase 3)
- 跨 spec cross-check 矩阵 / `lint --cross-check`(Phase 5)
- `lifecycle --format markdown` 的三段式摘要(Project Rule / Boundary / Test Binding Violations)(Phase 5)
- 把覆盖率纳入 `is_passing` 或 guard 门禁(本期明确不做)
- 多次运行取共识 / adversarial converge(Phase 8 audit)
- 真正调用模型后端的 inferential verdict(仍由 host 注入,见 project.spec)
- `test_found` 采用 cargo runner 语义(`cargo test -- --list` 建索引):v1 用精确函数名存在性,runner 语义留作后续(它需要编译,成本高)
- 跨 crate 的 package 限定精确解析(v1 在 `--code` 路径内按函数名匹配,package 限定仅记录不做跨 crate 精确定位)
- per-row `boundary_relevant` 列(边界检查仍是单条 synthetic scenario,不下沉成每行属性)
