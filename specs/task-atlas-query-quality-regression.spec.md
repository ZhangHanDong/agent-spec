spec: task
name: "Atlas Query Quality Regression Loop"
tags: [atlas, evaluation, query, regression, dogfood]
satisfies: [REQ-ATLAS-QUERY-QUALITY-REGRESSION]
depends: [task-atlas-agent-evaluation, task-atlas-explore-flow-impact]
estimate: 2d
---

## Intent

把 Atlas 查询质量从人工 rubric 提升为可回归的机器评分：在现有 benchmark evaluator 中加入
两层 query corpus、严格 observation 输入和版本化 score receipt，使错误 symbol/path、forbidden
结果、证据缺失及 stale/capability 隐藏都会形成明确失败，同时保持真实仓库和 Agent 执行 opt-in。

<!-- lint-ack: bdd-rule-grouping — corpus validation、scoring 与 CLI receipt 是同一条回归链 -->

## Decisions

- 保留 `agent-spec/atlas-eval/corpus-v1` 与 E0 命令兼容；新增
  `agent-spec/atlas-eval/query-corpus-v1`、`query-results-v1` 和
  `query-regression-v1`，不得把 query golden 字段塞进旧 run-plan schema。
- `benchmarks/atlas/query-corpus.json` 同时包含 `deterministic-fixture` 与
  `pinned-repository` case；后者使用 40 位 Git revision 并通过 `paired_fixture`
  链接前者。默认测试加载已提交 corpus/results，并以当前离线 fixture 的 search/flow
  输出替换对应 observation；不访问外部仓库或网络。
- observation 按 rank 顺序记录 canonical symbols，另记录完整 symbol path、evidence labels、typed
  diagnostics、response bytes、duration、read-back 和 follow-up query。
- score receipt 同时保留 per-case correctness 与 aggregate metrics。symbol recall、MRR、path
  precision/recall、evidence recall 使用确定性公式；forbidden hit、缺失 required diagnostic 或
  超过 ambiguity allowance 都是 correctness failure。
- 默认测试从真实 `rust_atlas::search` 与 `rust_atlas::flow` 输出生成 fixture observation，再经
  同一 scorer 验证；不得只对手写的静态 result 文件自洽评分。
- 新命令为 `agent-spec atlas benchmark score --corpus <path> --results <path> [--out <path>]`；
  `--out` 使用现有 atomic JSON writer，失败时不产生 partial receipt。
- corpus 与 results 使用 typed serde、`deny_unknown_fields`、唯一 case id、严格 schema/version
  配对；不得使用字符串搜索代替结构化解析。
- `allowed_ambiguity` 同时计算额外 symbol 与 path，范围固定为 `0..=64`；forbidden hit 不受
  allowance 豁免。required diagnostic 按 `{kind, code}` 精确匹配。

## Boundaries

### Allowed Changes
- benchmarks/atlas/**
- src/atlas_eval.rs
- src/main.rs
- docs/atlas-evaluation.md
- docs/atlas-roadmap.md
- knowledge/requirements/req-atlas-query-quality-regression.md
- specs/task-atlas-query-quality-regression.spec.md
- README.md
- AGENTS.md
- skills/agent-spec-tool-first/**
- CHANGELOG.md

### Symbols
- rust-atlas: agent_spec::atlas_eval

### Forbidden
- 不改变旧 E0 corpus、run plan 或 receipt 的解析兼容性
- 不在默认测试、score 命令或 corpus validation 中 clone、fetch 或启动真实 Agent
- 不用单一 expected-symbol hit 代替 path、evidence、diagnostic 与 forbidden 检查
- 不复制 CodeGraph 的 benchmark 百分比作为 Atlas pass threshold
- 不把 runtime hint 或 candidate edge 作为 lifecycle/KLL 的确定性证明

## Out of Scope

- 自动执行 pinned repository query
- 运行真实模型 A/B
- 改变默认 MCP surface
- 实现新的 ranking、traversal 或 dynamic-dispatch mechanism

## Completion Criteria

场景: 两层 query corpus 经过严格验证
  测试: test_atlas_query_checked_in_corpus_has_fixture_and_pinned_repository_tiers
  Level: integration
  假设 checked-in corpus 包含确定性 fixture 与真实 Rust repository case
  当 query corpus validator 读取它
  那么 每个 pinned case 使用 full Git revision 并链接一个 fixture case
  并且 默认测试不执行外部仓库或网络操作

场景: scorer 计算 retrieval、path 与 query cost 指标
  测试: test_atlas_query_score_computes_recall_mrr_paths_and_costs
  Level: unit
  假设 observation 包含有序 symbols、paths、evidence、diagnostics 和 query cost
  当 scorer 生成 regression receipt
  那么 receipt 包含 symbol recall、MRR、path precision/recall、forbidden-hit rate、evidence recall、response bytes、latency、read-back 与 follow-up query

场景: 错误 path 与 forbidden hit 阻塞 correctness
  测试: test_atlas_query_score_rejects_wrong_paths_forbidden_hits_and_missing_evidence
  Level: unit
  假设 observation 命中 expected symbol 但返回错误 path、forbidden symbol 且缺失 evidence
  当 scorer 评估该 case
  那么 case verdict 为 fail 并分别报告 path、forbidden 与 evidence 缺口

场景: required stale diagnostic 不得被隐藏
  测试: test_atlas_query_score_requires_declared_stale_diagnostic
  Level: unit
  假设 corpus case 声明 stale diagnostic 为 required
  当 observation 不包含要求的 exact stale diagnostic
  那么 case verdict 为 fail 并在 missing diagnostics 中列出 stale

场景: pinned repository 不得伪装为 fixture
  测试: test_atlas_query_corpus_rejects_fixture_path_as_pinned_repository
  Level: unit
  假设 pinned-repository case 使用 `fixtures/` 下的 repository 路径
  当 query corpus validator 读取它
  那么 返回 `atlas-query-corpus-pinned-repository`
  并且 不接受该 case 作为真实仓库证据

场景: observation 集合必须与 corpus 一一对应
  测试: test_atlas_query_score_rejects_duplicate_missing_and_unknown_observations
  Level: unit
  假设 results 重复、遗漏或增加 case id
  当 scorer 验证 results
  那么 返回稳定 diagnostic 且不生成 regression receipt

场景: ambiguity allowance 必须有界
  测试: test_atlas_query_corpus_rejects_unbounded_ambiguity
  Level: unit
  假设 一个 case 声明 `allowed_ambiguity=65`
  当 query corpus validator 读取它
  那么 返回 `atlas-query-corpus-ambiguity`
  并且 不接受该 golden case

场景: 嵌套 corpus 与 observation 字段保持严格
  测试: test_atlas_query_corpus_and_results_reject_nested_unknown_fields
  Level: unit
  假设 required diagnostic 或 observed diagnostic 含未知字段
  当 typed loader 解析 JSON
  那么 分别返回 query corpus 或 results parse diagnostic
  并且 未知字段不被静默忽略

场景: score CLI 原子输出版本化 receipt
  测试: test_atlas_benchmark_score_cli_writes_atomic_receipt
  Level: integration
  假设 checked-in corpus、results 与不存在的 `--out` 文件
  当 `atlas benchmark score` 执行
  那么 stdout 为空且目标文件是绑定 corpus version 的完整 `query-regression-v1` JSON

场景: 当前 search 与 flow 输出通过 live fixture regression
  测试: test_atlas_query_live_fixture_probe_scores_current_search_and_flow
  Level: integration
  假设 fixture graph 从 checked-in Rust source 与 SCIP index 离线构建
  当 当前 `search` ranking 与 `flow` path 被投影为 observation
  那么 同一 query scorer 接受这些实时结果
  并且 response bytes 来自实际序列化的查询响应

场景: correctness 失败写出 receipt 后阻塞 CLI
  测试: test_atlas_benchmark_score_cli_fails_after_writing_regression_receipt
  Level: integration
  假设 observation 集合结构合法但一个 case 缺失 expected symbol
  当 `atlas benchmark score --out` 执行
  那么 原子输出保留 corpus fingerprint、typed diagnostics 和失败 case
  并且 命令返回 `atlas-query-regression` 非零错误
