spec: task
name: "Atlas Worktree Identity and Layered Freshness"
tags: [atlas, worktree, freshness, scip, lifecycle]
satisfies: [REQ-ATLAS-WORKTREE-FRESHNESS]
depends: [task-atlas-edge-evidence-index, task-atlas-kll-integration]
estimate: 1w
---

## Intent

为 Atlas 建立唯一的 graph identity 与 layered freshness 契约，使 query、binding、
lifecycle、trace 和 MCP 对“这张图属于哪个 worktree、syn/SCIP/MIR 各自是否新鲜”给出
一致答案。借用其他 worktree 的图或 source 新于 SCIP index 时，不得继续生成确定性证据。

<!-- lint-ack: bdd-rule-grouping — identity、layer status 与消费方阻塞按同一状态契约线性验收 -->

## Decisions

- `Meta` 增加 `GraphIdentity`：canonical repository root、optional git common dir、worktree
  root、canonical graph root、`rustc -Vv` toolchain identity；无 git 时 worktree root 等于
  canonical code root，common dir 为 null。
- `LayerStatus` 固定为 `fresh|stale|unavailable`；`AtlasStatus` 分别携带 syn/scip/mir 的
  state、fingerprint、diagnostics 和 syn stale files。
- explicit `atlas build --scip` 保存 SCIP index fingerprint 与当时的 source-set
  fingerprint；自动 refresh 复用旧 index 时不得更新后者，因此源码改动后 scip 保持 stale。
- `status` 是 freshness 的唯一 library 入口；既有 `check` 兼容包装 syn stale files，
  query/tree/refs/impls/search 与 MCP 都嵌入同一个 `AtlasStatus`。
- query 发现 worktree mismatch 时返回 `AtlasError::WorktreeMismatch`，包含 recorded/current
  path，不返回 node/edge；`atlas status` 本身必须成功返回 mismatch diagnostic 供修复。
- schema-valid Meta 必须先计算 shared status 并拒绝 worktree mismatch，再加载 query index；
  同一 worktree 的 missing/corrupt/stale index 仍必须先于自动 refresh 返回 typed error。
- `capability.scip=false` 且没有 semantic authority metadata 才表示 unavailable；声明 SCIP
  capability 却缺少 index、index fingerprint 或 source-set fingerprint，以及 capability=false
  却残留上述字段时，都必须 fail closed 为 stale。
- provider fingerprint 按 schema、recorded graph identity、explicit toolchain identity、recorded
  source-set fingerprint、graph fingerprint 与每层 recorded fingerprint 的 typed canonical JSON
  计算 blake3；排除 current identity、diagnostics、stale files 与显示顺序。
- `AtlasProvider`、binding 与 Contract symbol verifier 共用 `status` 与 authority gate，不能各自
  重新解释 stale。
- graph load 的 schema mismatch 继续优先失败；不得把 schema mismatch 降级为 stale。

## Boundaries

### Allowed Changes
- crates/rust-atlas/**
- src/spec_knowledge/code_graph.rs
- src/spec_verify/atlas_symbols.rs
- src/spec_mcp/**
- src/main.rs
- fixtures/atlas/**
- knowledge/requirements/req-atlas-worktree-freshness.md
- specs/task-atlas-worktree-layered-freshness.spec.md
- docs/atlas-roadmap.md
- docs/intent-compiler/**
- README.md
- AGENTS.md
- skills/agent-spec-tool-first/**
- CHANGELOG.md

### Forbidden
- 不把绝对 worktree path 写入 KLL 或 Task Contract
- 不把 syn fresh 推断成 SCIP/MIR fresh
- 不在 worktree mismatch 下生成 binding、trace target 或 symbol verdict
- 不要求 git 才能构建或查询 Atlas
- 不通过自动重跑 rust-analyzer 隐藏 stale SCIP

## Out of Scope

- watch/daemon
- MIR extractor 本身
- 自动创建或删除 git worktree
- 非 Rust provider freshness 实现

## Completion Criteria

场景: 同一 worktree 的显式 SCIP build 报告独立 layer
  测试: test_atlas_status_reports_fresh_syn_scip_and_unavailable_mir
  假设 当前 source-set 与显式供应的 SCIP index 匹配
  当 `atlas status` 在 build worktree 中运行
  那么 `LayerStatus` 将 syn 与 scip state 序列化为 `fresh`
  并且 将 mir state 序列化为 `unavailable`，且不会错误报告 `stale`

场景: 无 git 项目使用 code root identity
  测试: test_atlas_identity_falls_back_outside_git
  假设 fixture 不在 git repository 中
  当 Atlas build 与 query 运行
  那么 worktree root 等于 canonical code root
  并且 git common dir 为 null

场景: 借用另一个 worktree 的图被拒绝
  测试: test_atlas_rejects_borrowed_worktree_graph
  假设 同一 git repository 有两个 linked worktree 且图由第一个构建
  当 第二个 worktree 对该 graph dir 执行 definitive query
  那么 返回 `atlas-worktree-mismatch`
  并且 error 同时命名 recorded 与 current worktree

场景: source refresh 不伪造 SCIP fresh
  测试: test_atlas_refresh_keeps_reused_scip_layer_stale
  假设 source 与 SCIP 初始都 fresh
  当 source 改动触发自动 syn refresh 并复用旧 SCIP index
  那么 syn state 为 fresh 且 scip state 为 stale
  并且 scip diagnostic 命名 source-set fingerprint mismatch

场景: stale semantic layer 阻塞 binding
  测试: test_code_bindings_block_on_stale_semantic_layer
  假设 Contract symbol 的唯一证据来自 stale SCIP layer
  当 binding construction 消费 Atlas provider
  那么 binding construction 失败并返回 `atlas-stale`
  并且 不返回可序列化的 definitive binding

场景: schema mismatch 优先于 freshness
  测试: test_atlas_rejects_mismatched_schema_version
  假设 graph meta 使用旧 schema version
  当 status、query 或 binding 尝试加载图
  那么 返回 `atlas-schema-mismatch` rebuild diagnostic
  并且 不返回 layered freshness 或 definitive evidence

场景: 所有 read result 共用完整 status 与兼容 stale mirror
  测试: test_atlas_read_results_share_status_and_stale_mirror
  假设 tree、query、refs、impls 与 search 消费同一份 fresh、frozen stale 或 refreshed graph
  当 library 序列化每一种 read result
  那么 每个 result 都携带相等的 `AtlasStatus`
  并且 顶层 stale 精确等于 `status.syn.stale_files`

场景: 每个 read consumer 独立 refresh 后重算 status
  测试: test_atlas_each_read_consumer_recomputes_status_after_refresh
  假设 tree、query、refs、impls 与 search 各自调用前都有新的 source edit
  当 每个 consumer 分别执行 non-frozen read
  那么 每个返回结果的 syn state 都是 fresh 且 SCIP state 仍是 stale
  并且 每个顶层 stale 都精确镜像其返回 status 的 syn stale files

场景: provider fingerprint 精确覆盖 authority inputs
  测试: test_atlas_provider_fingerprint_exact_inputs_and_exclusions
  假设 typed canonical payload 固定了 recorded authority inputs
  当 计算 provider fingerprint 并逐项改变 schema、recorded identity、toolchain、source-set、graph 或 layer fingerprint
  那么 hash 等于固定 expected value 且每个 included input 的改变都会改变 hash
  并且 current identity、diagnostics、stale files 与 display ordering 的改变不会改变 hash

场景: CLI 与 MCP status 直接序列化 library shape
  测试: test_atlas_status_cli_and_mcp_serialize_library_shape
  假设 CLI、MCP 与 library status 指向同一个 Atlas graph
  当 `atlas status` 与 MCP `atlas_status` 分别返回 JSON
  那么 两者都精确等于 `rust_atlas::status` 的序列化 shape
  并且 不包含 consumer-local freshness interpretation

场景: lifecycle 拒绝 stale semantic symbol authority
  测试: test_lifecycle_reports_stale_atlas_semantic_layer
  假设 syn 已 refresh 为 fresh 而可用 SCIP authority 仍 stale
  当 Contract symbol verifier 执行 lifecycle validation
  那么 scenario verdict 不得使用 stale semantic evidence
  并且 evidence 返回共享 authority gate 的 `atlas-stale` diagnostic

场景: stale non-frozen indexed reads 先验证 query index
  测试: test_atlas_stale_non_frozen_queries_validate_index_before_refresh
  假设 同一 worktree 的 source stale 且 query index missing、旧版、stale 或 corrupt
  当 query、refs、impls 或 search 执行 non-frozen read
  那么 返回对应 typed query-index error 且不自动修复
  并且 source 与 graph bytes 保持不变

场景: partial SCIP authority 必须 fail closed
  测试: test_atlas_partial_scip_authority_fails_closed
  假设 SCIP capability 缺少任一 required authority field 或 disabled capability 残留 authority field
  当 status 与 shared authority gate 读取该 schema-valid Meta
  那么 SCIP layer 为 stale 且 diagnostic 确定性命名不一致字段
  并且 authority gate 返回 `atlas-stale`；只有无残留 metadata 的 disabled SCIP 为 unavailable

场景: command-level binding failure 保留既有 artifact bytes
  测试:
    过滤: test_atlas_requirements_bind_rejects_stale_semantic_authority_without_writing
    层级: integration
  假设 explicit SCIP build 经 syn refresh 后具有 fresh syn、stale SCIP，且输出文件已有 sentinel bytes
  当 实际 `RequirementCommands::Bind` 路径运行到最终 filesystem write boundary
  那么 command 返回 `atlas-stale`
  并且 请求的 output 仍与 sentinel byte-identical

场景: borrowed worktree mismatch 优先于无效 query index
  测试:
    过滤: test_atlas_borrowed_worktree_mismatch_precedes_invalid_query_index
    层级: integration
  假设 graph 来自另一个 linked worktree 且 query index missing 或 corrupt
  当 borrowed worktree 执行 non-frozen indexed query
  那么 返回同时命名 recorded/current path 的 `atlas-worktree-mismatch`
  并且 graph bytes 保持不变且不会尝试 index repair
