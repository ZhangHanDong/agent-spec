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
- provider fingerprint 按 schema、identity、toolchain、source-set、SCIP/MIR fingerprint
  的 canonical JSON 计算 blake3；`AtlasProvider`、binding 与 Contract symbol verifier
  共用 `status`，不能各自重新解释 stale。
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
  当 `requirements bind` 消费 Atlas provider
  那么 binding 失败并返回 `atlas-stale`
  并且 不写 code-bindings artifact

场景: schema mismatch 优先于 freshness
  测试: test_atlas_rejects_mismatched_schema_version
  假设 graph meta 使用旧 schema version
  当 status、query 或 binding 尝试加载图
  那么 返回 `atlas-schema-mismatch` rebuild diagnostic
  并且 不返回 layered freshness 或 definitive evidence
