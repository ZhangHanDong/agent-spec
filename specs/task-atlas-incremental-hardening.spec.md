spec: task
name: "Atlas Incremental Hardening"
tags: [atlas, incremental, generation, frontier, recovery, dogfood]
satisfies: [REQ-ATLAS-INCREMENTAL-HARDENING]
depends: [task-atlas-edge-evidence-index, task-atlas-worktree-layered-freshness]
estimate: 5d
---

## Intent

把 Rust Atlas 的 hash 增量刷新升级为 committed-generation transaction：reader 固定读取一个
完整 generation，build 只重算 bounded dependent frontier，中断工作由 orphan queue 恢复，
健康 zero-change build 不再支付全图 resolution/validation 与 authority rewrite 成本。

<!-- lint-ack: bdd-rule-grouping — generation、frontier、recovery 与 fast path 共同定义一个不可拆分的增量 authority transaction -->

## Decisions

- graph root 新增 versioned `CURRENT.json` 与 `generations/<id>/generation.json`；没有 pointer
  的旧布局继续只读，首次成功 build 再迁移。
- query/status/load 在入口固定一次 generation data path；identity 的 graph root 仍是用户传入的
  graph root，不把 generation directory 当成另一个 worktree graph。
- staging 优先 hard-link unchanged shard，失败时 copy；所有 staged shard write 使用 atomic
  replace，禁止通过 hard link 原地改写 active inode。
- generation manifest 记录 meta、query-index、shard artifact digest、graph/capability/input-plan
  fingerprint 与 base generation；pointer 只在 manifest 完整后原子替换。
- 本事务持有的 uncommitted staging 可幂等清理且不得删除 committed generation；跨进程遗留
  staging 与旧 generation 回收等待 D3 single-writer/reader-retention contract。
- Cargo input-plan cache key 使用 manifest content hash、`rustc -vV`、features、target/cfg、
  provider version 与 schema；source module ownership 每次从当前 Rust module tree 重建，query
  触发的 stale refresh 继承已提交的 features、target 与 cfg。
- frontier 包含 changed shard、removed target id dependent、changed canonical/bare target dependent
  与 impl dependent；默认上限 2048 shards，超限从 clean staging 重新执行 full resolution。
- orphan queue 在 resolution 前原子写入，bounded 且 path-safe；resolved、external 与 deterministic
  unresolved 都消费，只有 pointer commit 后清空。前一轮 orphan 总在 zero-change fast path 前合并。
- resolution/validation 默认 batch 256 shards，working-byte ceiling 默认 536870912 bytes，并覆盖
  source、serialized shard 与 overlay admission；library cancellation token 在 extraction、
  resolution、overlay、validation 与 publication 前检查。
- zero-change fast path 要求 source、input plan、requested capability、committed manifest 全匹配且
  orphan 为空；它不创建 staging，不调用 resolution/validation，不写任何 authority/control file。
- maintenance 在 commit 后 best-effort；失败只进入 report warning。
- D2 report 增加 generation、frontier、edge delta、bounded bytes、cache、orphan 与 fallback 字段；
  timing/RSS 只作观察，不作为 deterministic gate。

## Boundaries

### Allowed Changes
- crates/rust-atlas/Cargo.toml
- crates/rust-atlas/src/**
- fixtures/atlas/incremental-hardening/**
- benchmarks/atlas/**
- src/atlas_eval.rs
- src/main.rs
- docs/atlas-roadmap.md
- docs/atlas-evaluation.md
- docs/atlas-incremental-builds.md
- docs/superpowers/plans/2026-07-20-atlas-d2-incremental-hardening.md
- knowledge/requirements/req-atlas-incremental-hardening.md
- specs/task-atlas-incremental-hardening.spec.md
- .agent-spec/wiki/**
- README.md
- AGENTS.md
- skills/agent-spec-tool-first/**
- CHANGELOG.md

### Symbols
- rust-atlas: rust_atlas::build

### Forbidden
- 不让 reader 在一次操作内跨 committed generation
- 不原地修改 active generation artifact
- 不在 frontier overflow 后继续发布 partial resolution
- 不把 deterministic unresolved 留作永久 orphan
- 不在 pointer commit 前清理 orphan queue
- 不把 mtime 当 input-plan authority
- 不因 maintenance failure 撤销已提交 generation
- 不在 D2 启用 watcher、daemon、retry scheduler 或 pending watermark

## Out of Scope

- D3 filesystem watcher、pending event 与 daemon lifecycle
- 官方 rustc MIR producer
- 默认 MCP surface 变化
- 用 timing 或 RSS 的绝对值作为跨机器 correctness gate
- 非 Rust provider 的增量实现

## Completion Criteria

场景: query 固定读取一个 committed generation
  测试:
    过滤: test_atlas_reader_pins_one_generation_during_pointer_change
    层级: integration
  假设 query 开始后另一个 build 已准备发布新 pointer
  当 test hook 在 meta 与 index read 之间切换 pointer
  那么 query 的 meta、shards 与 index generation id 保持相同

场景: generation publish failure 保留 baseline
  测试:
    过滤: test_atlas_generation_failures_keep_committed_baseline
    层级: integration
  假设 baseline graph 可查询且 staging、rename 与 pointer write 可分别注入失败
  当 每个 failure point 执行 incremental build
  那么 baseline query JSON 与 artifact digest set byte-identical

场景: legacy graph 只在成功 build 后迁移
  测试:
    过滤: test_atlas_legacy_generation_migrates_only_after_success
    层级: integration
  假设 graph 使用无 `CURRENT.json` 的 legacy layout
  当 第一轮 build 失败而第二轮成功
  那么 第一轮后 legacy query 仍通过且第二轮后 pointer 与 manifest 完整

场景: transaction-owned staging cleanup 幂等且保留 active generation
  测试:
    过滤: test_atlas_owned_staging_cleanup_is_idempotent_and_preserves_active_generation
    层级: integration
  假设 active generation 与一个未提交 transaction staging 同时存在
  当 staging cleanup 重复运行
  那么 staging 保持不存在且 active generation 仍 byte-identical

场景: Cargo input plan 使用 content key
  测试:
    过滤: test_atlas_input_plan_uses_content_not_manifest_mtime
    层级: integration
  假设 manifest 先只改 mtime再保持 mtime改 bytes
  当 两次 incremental build 运行
  那么 第一轮 report 为 cache hit 且第二轮为 cache miss

场景: cached Cargo plan 不缓存 source module ownership
  测试:
    过滤: test_atlas_input_plan_rebuilds_source_module_ownership
    层级: integration
  假设 Cargo inputs 不变但 source path attribute 移动 module
  当 incremental build 命中 Cargo plan cache
  那么 graph 只保留新的 canonical module symbol 与 structural id

场景: automatic refresh 保留 committed Cargo inputs
  测试:
    过滤: test_atlas_auto_refresh_preserves_input_plan_configuration
    层级: integration
  假设 baseline graph 使用显式 features 与 cfg 构建
  当 source stale 后非 frozen query 自动 refresh
  那么 新 input-plan 保留原 configuration 与 fingerprint

场景: declaration rename 重算 unchanged caller
  测试:
    过滤: test_atlas_incremental_frontier_repairs_renamed_target_dependents
    层级: integration
  假设 unchanged caller 精确调用另一个 file 的 declaration
  当 callee rename 后 incremental build
  那么 caller edge 变为 unresolved 且 touched shards 同时包含 caller 与 callee

场景: bare candidate set 变化重算 unchanged caller
  测试:
    过滤: test_atlas_incremental_frontier_repairs_bare_name_ambiguity
    层级: integration
  假设 unchanged caller 的 bare target 初始唯一
  当 新 file 加入同名 declaration
  那么 caller edge 不再保留 obsolete exact target

场景: dependency frontier overflow 显式 full fallback
  测试:
    过滤: test_atlas_incremental_frontier_overflow_falls_back_completely
    层级: integration
  假设 changed symbol 的 reverse dependents 超过 configured frontier limit
  当 incremental build 运行
  那么 report fallback 为 `dependency-frontier-overflow` 且完整 graph validation 通过

场景: zero-change build 恢复 orphan queue
  测试:
    过滤: test_atlas_zero_change_build_recovers_interrupted_orphans
    层级: integration
  假设 前一轮 resolution 在持久化 queue 后中断
  当 source 没有额外变化且 build 再次运行
  那么 orphan 全部被处理、完整 generation 发布且 queue 被删除

场景: deterministic unresolved 消费 orphan item
  测试:
    过滤: test_atlas_orphan_recovery_consumes_deterministic_unresolved
    层级: integration
  假设 orphan target 在完整 symbol index 中仍不可解析
  当 recovery build 分类该 target
  那么 committed edge 为 unresolved 且 orphan queue 为空

场景: healthy zero-change build 不执行 graph work
  测试:
    过滤: test_atlas_zero_change_fast_path_is_byte_and_counter_inert
    层级: integration
  假设 generation、source、input plan、capability 全 fresh 且无 orphan
  当 非 full build 重复执行
  那么 generation 与 artifact bytes 不变且 resolution/validation counter 都为零

场景: cancellation 只丢弃 staging
  测试:
    过滤: test_atlas_incremental_cancellation_preserves_generation_and_orphans
    层级: integration
  假设 token 在一个 resolution batch 后变为 cancelled
  当 build 观察 token
  那么 build 返回 cancelled、active generation 不变且 orphan queue 保留

场景: working-byte ceiling 在 publish 前失败
  测试:
    过滤: test_atlas_incremental_working_byte_limit_fails_closed
    层级: integration
  假设 configured byte ceiling 小于 frontier batch 的 deterministic serialized size
  当 incremental build 运行
  那么 resource diagnostic 指明 required 与 limit 且 pointer byte-identical

场景: semantic capability 与 edge generation 原子一致
  测试:
    过滤: test_atlas_incremental_overlay_capability_is_generation_atomic
    层级: integration
  假设 新 overlay 同时改变 semantic edges 与 capability fingerprint
  当 build 分别在 pointer 前失败和完整成功
  那么 reader 只观察完整 old pair 或完整 new pair

场景: post-commit orphan maintenance failure 保持可恢复
  测试:
    过滤: test_atlas_post_commit_orphan_clear_failure_remains_recoverable
    层级: integration
  假设 generation pointer 已提交但 orphan queue 删除失败
  当 build 返回 maintenance warning 且下一次 zero-change build 运行
  那么 已提交 generation 不回滚、queue 重绑定并在下一次 build 被恢复清除

场景: D2 fixture matrix 输出完整 receipt
  测试:
    过滤: test_atlas_incremental_acceptance_matrix_records_receipts
    层级: integration
  假设 checked-in cold、zero、edit、delete、manifest、overflow、overlay、cancel、failure 与 recovery cases
  当 evaluator 捕获 production build report
  那么 每个 receipt 含 touched shards、edge delta、bounded bytes、generation、cache、orphan 与 fallback 字段

场景: D2 docs 与 wiki 保留 authority 边界
  测试:
    过滤: test_atlas_incremental_docs_describe_generation_and_live_runtime_boundary
    层级: integration
  假设 reader docs、roadmap、skill 与 tracked wiki
  当 文档契约测试读取其 generation、frontier、orphan 与 zero-change terms
  那么 页面明确 D2 不启用 watcher 或 daemon 且 graph 仍是 derived authority
