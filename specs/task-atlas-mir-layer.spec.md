spec: task
name: "Atlas Optional MIR Overlay"
tags: [atlas, code-graph, mir, static-analysis]
satisfies: [REQ-ATLAS-MIR-OVERLAY]
depends: [task-atlas-edge-evidence-index, task-atlas-explore-flow-impact, task-atlas-worktree-layered-freshness]
risk: A
---

## Intent

在不污染 stable 默认构建的前提下，把版本化 MIR producer 产出的精确 calls 和函数 CFG
summary 叠加到现有 Atlas graph。MIR 是可逆的增强层；不可用、失败或陈旧时，Atlas 必须
保留 syn/SCIP 基线并明确暴露能力缺口。

## Decisions

- 2026-07-20 的评估结论：Charon 需要独立 nightly pin，未通过“支持仓库 stable toolchain
  且滞后不超过两个 minor”的门禁，因此不作为绑定依赖。
- MIR producer 使用独立进程和版本化 `rust-atlas/mir-overlay-v1` JSON 协议；目标 producer
  基于 `rustc_public`，但不进入默认 workspace dependency graph。
- 根 crate 和 `rust-atlas` 都提供非默认 `mir` Cargo feature；无该 feature 时 CLI 和公开
  build API 不能激活 overlay 或调用 producer。纯 JSON consumer 不依赖 compiler crate，
  参与默认编译和测试，并可在 refresh 时维持既有 overlay 的独立 freshness。
- `atlas build --mir <path>` 消费已有 overlay；`--mir-driver <binary>` 只以固定 argv
  `--code <root> --out <path>` 直接调用 producer，两个输入模式互斥。
- 函数 node 增加 optional CFG summary；edge 增加 default-false `generic`，不提升 schema
  version，旧的 schema v6 consumer 仍可忽略同版本可选字段。
- 重复 source-target relation 的查询视图按 `mir > scip > syn` 选择；canonical shard 保留
  所有 provenance evidence。
- 任一 producer/overlay 错误都转换为 `mir-extraction-failed` build diagnostic，并在写入
  meta/index 前清除旧 MIR edge 和 CFG。

## Boundaries

### Allowed Changes
- Cargo.toml
- crates/rust-atlas/**
- src/main.rs
- src/spec_knowledge/code_graph.rs
- src/spec_verify/atlas_symbols.rs
- fixtures/atlas/**
- knowledge/requirements/req-atlas-mir-overlay.md
- specs/task-atlas-mir-layer.spec.md
- specs/roadmap/task-atlas-mir-layer.spec.md
- docs/atlas-roadmap.md
- docs/atlas-mir-overlay.md
- docs/atlas-schemas/mir-overlay-v1.schema.json
- README.md
- AGENTS.md
- CHANGELOG.md

### Symbols
- rust-atlas: rust_atlas::build
- rust-atlas: rust_atlas::query
- rust-atlas: rust_atlas::status

### Forbidden
- 不让 nightly、Charon 或 rustc compiler crates 进入默认 dependency graph
- 不通过 shell 解释 driver 命令
- 不在 extraction 失败后保留旧 MIR evidence
- 不把 generic call 展开为 monomorphized instance 集合
- 不改变 intent-code provider 或 lifecycle symbol verification 的生产行为

## Out of Scope

- borrow-check、ownership 或完整 MIR body 持久化
- 跨 crate monomorphization
- 动态分派候选扩展（A4）
- 常驻 daemon/watch（D3）

## Completion Criteria

场景: MIR overlay 增加精确 calls 和 CFG
  测试: test_atlas_mir_overlay_adds_calls_edges
  假设 当前 source fingerprint 的 overlay 含 caller、callee、call site 和 CFG summary
  当启用 mir feature 构建 Atlas
  那么 shard 含 exact mir calls edge 且 caller node 含相同 CFG summary

场景: Query 选择最高 provenance
  测试: test_atlas_query_prefers_highest_provenance_edge
  假设同一 source-target relation 同时有 syn、scip 和 mir evidence
  当查询 caller
  那么 query view 只返回 mir relation 且 shard 仍保留全部 evidence

场景: 所有 indexed query consumer 共用 precedence
  测试: test_query_index_projects_highest_provenance_relations
  假设同一 call/reference relation 有 syn、SCIP 和 MIR evidence
  当构建 derived query index
  那么 refs、flow、impact 和 query 读取的 index 只保留 MIR projection

场景: MIR extraction failure 降级到基线
  测试: test_atlas_mir_failure_degrades_to_syn_graph
  假设 graph 曾含 MIR facts 且下一次 producer 非零退出或 overlay 无效
  当再次构建 Atlas
  那么命令退出 0、diagnostics 含 mir-extraction-failed 且 graph 不含 mir provenance 或 CFG

场景: Stale MIR 独立可见
  测试: test_atlas_status_reports_stale_mir_after_syn_refresh
  假设有效 MIR overlay 后 source 文件发生变化并只刷新 syn
  当执行 atlas status
  那么 syn 为 fresh、mir 为 stale 且 diagnostic 含 recorded/current source fingerprint

场景: Generic call 保留泛型形式
  测试: test_atlas_mir_generic_call_is_not_monomorphized
  假设 overlay 含一个 generic target 和 generic=true
  当叠加 MIR facts
  那么 graph 只有泛型 target edge 且 edge.generic 为 true

场景: 错误 schema 不写入部分 overlay
  测试: test_atlas_mir_rejects_schema_without_partial_write
  假设 overlay schema 不是 rust-atlas/mir-overlay-v1
  当构建 Atlas
  那么 diagnostics 含 mir-extraction-failed 且所有 shard 不含部分 MIR facts

场景: 跨 shard 写失败保留完整基线
  测试: test_atlas_mir_generation_write_failure_keeps_baseline
  假设 MIR overlay 同时修改多个 shard 且 staging 的第二次写入失败
  当提交新的 shard generation
  那么当前 generation 的所有 baseline shard 保持 byte-identical 且没有部分 MIR facts

场景: Nested wire 字段严格校验
  测试:
    过滤: test_atlas_mir_rejects_unknown_nested_wire_fields
    层级: unit
    命中: rust-atlas/mir-overlay-v1 nested wire objects
  假设 extractor、CFG 或 call site 含协议 schema 未声明的字段
  当 consumer 解析 overlay
  那么 build 返回 mir-extraction-failed 且不接受该 artifact

场景: Default build 不需要 MIR
  测试: test_atlas_default_build_excludes_mir
  假设 stable toolchain 和默认 Cargo features
  当不传 MIR options 构建 Atlas
  那么构建成功、producer invocation count 为零且 MIR capability 为 unavailable

场景: Driver 使用固定 argv
  测试:
    过滤: test_atlas_mir_driver_uses_fixed_argv_and_degrades_on_failure
    层级: integration
    替身: temporary executable argv recorder
  假设一个记录 argv 后非零退出的测试 producer
  当 atlas build 使用 --mir-driver
  那么记录值严格为 --code、canonical root、--out、overlay path 且 build 返回 mir-extraction-failed
