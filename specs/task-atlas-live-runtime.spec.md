spec: task
name: "Atlas Live Runtime"
tags: [atlas, live, watcher, daemon, incremental, dogfood]
satisfies: [REQ-ATLAS-LIVE-RUNTIME]
depends: [task-atlas-incremental-hardening, task-atlas-worktree-layered-freshness, task-atlas-query-quality-regression]
estimate: 5d
---

## Intent

在 D2 的增量 generation transaction 上增加可选、可恢复的 watch/daemon runtime，缩短 Rust
worktree 的 stale window。后台模式必须保留事件水位、单 writer、typed degraded、reader lease
和 no-daemon correctness，不得成为第二份图 authority 或默认 MCP 依赖。

<!-- lint-ack: bdd-rule-grouping — watermark、daemon identity、query status 与 reclamation 是同一个 live-runtime 状态机 -->

## Decisions

- `crates/rust-atlas/src/scope.rs` 是 build 与 watcher 的唯一输入范围策略；`.rs`、workspace
  manifests、`Cargo.lock`、`.cargo/config.toml`、toolchain 与项目配置进入范围，gitignore、workspace excludes、`target/`、
  graph runtime 和 canonical root 外路径被拒绝。
- 使用 `notify 8.2`。macOS/Windows 创建一个 root recursive watch；Linux 创建 canonical-sorted
  non-recursive directory watches，默认上限 50000，cap/backend exhaustion 必须报告 partial/degraded。
- `.runtime/pending.json` 使用严格 schema 和单调 `u64` sequence。sync 只清除成功提交前 snapshot
  watermark 内且未出现在 `BuildReport.unparsed` 的事件；失败、degraded 和 mid-sync 新事件全部保留。
- lock contention 与 ordinary sync failure 分开计数；默认 `max_attempts=5`、`base_delay_ms=100`、
  `max_delay_ms=30000`，指数退避使用 `base * 2^(attempt-1)` 后取上限，成功清零。
- 使用 `fs2 0.4` advisory lock。每次 build 持有短期 `build.lock`；daemon lifetime 持有独立
  `daemon.lock`。锁竞争返回 typed retryable error，不能创建 staging 或移动 `CURRENT.json`。
- daemon 控制协议仅监听 `127.0.0.1`，registry/handshake 固定包含 canonical worktree、canonical
  graph、PID、start timestamp、128-bit startup nonce、agent-spec version、Atlas schema version 与 endpoint。
  PID 或 registry file 单独不能证明 identity。
- CLI 固定提供 `atlas daemon start|serve|status|sync|stop`。`serve` 是可监督 foreground 入口；
  `start` 只生成 detached local process；query 不经过 daemon socket。client disconnect 不停止 daemon。
- live runtime 状态固定为 `warming|healthy|pending|degraded|unavailable`。全局 status 返回全部 pending；
  query status 只把 result 代表的 source files 与 pending 相交。
- MCP discovery 保持纯静态且默认 tool list 不变；MCP Atlas tools 继续 frozen local reads，只通过
  现有 result status 看见 pending/degraded。
- query 通过 process-bound shared file lease 固定 generation。reclaimer 必须持 build writer lock，
  只能删除 unlocked stale lease、abandoned staging 和未被 current/active lease 引用的旧 generation。
  unreadable/ambiguous lease fail closed；maintenance failure 只产生 warning。
- runtime state 位于 graph root `.runtime/`，不进入 generation manifest、graph fingerprint、KLL、
  binding、lifecycle 或 archive authority。

## Boundaries

### Allowed Changes
- Cargo.toml
- Cargo.lock
- crates/rust-atlas/Cargo.toml
- crates/rust-atlas/src/**
- src/atlas_daemon.rs
- src/main.rs
- src/spec_mcp/**
- src/spec_knowledge/code_graph.rs
- fixtures/atlas/live-runtime/**
- benchmarks/atlas/**
- docs/atlas-live-runtime.md
- docs/atlas-incremental-builds.md
- docs/atlas-evaluation.md
- docs/atlas-roadmap.md
- docs/superpowers/specs/2026-07-20-atlas-live-runtime-design.md
- docs/superpowers/plans/2026-07-20-atlas-live-runtime.md
- knowledge/requirements/req-atlas-live-runtime.md
- specs/task-atlas-live-runtime.spec.md
- .agent-spec/wiki/**
- README.md
- AGENTS.md
- skills/agent-spec-tool-first/**
- CHANGELOG.md

### Symbols
- rust-atlas: rust_atlas::build
- rust-atlas: rust_atlas::status

### Forbidden
- 不让 daemon 代理或改变图查询、ranking、path 或 binding authority
- 不在失败、锁竞争、取消或 degraded 时清除 pending event
- 不基于 PID 或 registry file 单独附着 daemon
- 不在没有 writer lock 或 active reader 证明时删除 staging/generation
- 不监听 loopback 之外地址，不访问远程服务，不调用 LLM
- 不写用户 Agent 配置，不默认新增 MCP tool

## Out of Scope

- 远程、多主或跨机器 daemon
- graph database server 或 query RPC
- 非 Rust provider watch policy
- 修改默认 MCP explore/search 暴露策略
- A4.2 mechanism enricher、B5 context compiler 或 E1 Agent A/B

## Completion Criteria

Scenario: build 与 watcher 使用同一输入 scope
  Test:
    Filter: test_atlas_live_scope_is_shared_by_build_and_watcher
    Level: integration
  Given source、Cargo input、ignored、target、graph runtime、workspace excluded 与 outside-root paths
  When build walk 与 watcher classifier 处理相同路径
  Then 两者对 Atlas input 的接受结果完全一致

Scenario: 平台 watch plan 有确定上限
  Test:
    Filter: test_atlas_watch_plan_is_bounded_per_platform
    Level: unit
  Given macOS、Windows 与 Linux platform policy 和超过上限的目录集合
  When watcher 生成安装计划
  Then 前两者只有一个 recursive root 且 Linux 只保留 canonical first 50000 directories 并报告 partial coverage

Scenario: mid-sync event 不被旧 watermark 清除
  Test:
    Filter: test_atlas_pending_watermark_preserves_mid_sync_events
    Level: integration
  Given sync 已 snapshot 一个 pending event
  When 同一路径或另一输入在 commit 前获得更大 sequence
  Then acknowledge 只清除 snapshot 之前的 latest sequence 且新事件保留

Scenario: 失败 sync 保留 pending
  Test:
    Filter: test_atlas_failed_sync_preserves_pending_events
    Level: integration
  Given pending source 与 Cargo-input events
  When lock、cancel、extractor 与 I/O failure 分别结束 sync
  Then pending file bytes 和 active generation pointer 保持不变且 committed build 的 unparsed path 仍 pending

Scenario: query 只报告相关 pending path
  Test:
    Filter: test_atlas_live_status_scopes_pending_paths_to_query
    Level: integration
  Given pending set 同时包含 query result 内外的 source files
  When symbol、flow 与 impact result 生成 live status
  Then local stale 只包含结果代表的 pending files 且 global status 保留完整集合

Scenario: retry budget 分离并在超限后 degraded
  Test:
    Filter: test_atlas_retry_budget_degrades_without_dropping_pending
    Level: unit
  Given lock 与 ordinary failure 各自达到默认 5 次 retry
  When scheduler 计算 backoff 和下一状态
  Then delay 不超过 30000ms、两类计数互不覆盖、状态 degraded 且 pending 未清空

Scenario: transient failure 成功后清零
  Test:
    Filter: test_atlas_retry_state_resets_after_successful_sync
    Level: integration
  Given 两类 failure counters 均小于上限
  When 后续 D2 generation 成功提交
  Then counters、last error 与 degraded reason 全部清空并按 watermark acknowledge

Scenario: writer contention 不创建 graph work
  Test:
    Filter: test_atlas_writer_lock_allows_only_one_build
    Level: integration
  Given 一个进程持有 graph build writer lock
  When 第二个 build 尝试启动
  Then 返回 retryable writer-busy error 且 staging、CURRENT 与 pending bytes 不变

Scenario: dead writer 自动释放
  Test:
    Filter: test_atlas_dead_writer_lock_is_recoverable
    Level: integration
  Given 原 writer file handle 已关闭但 lock file 保留
  When 新 build writer 获取同一 advisory lock
  Then 获取成功且不使用旧 PID metadata 作为 liveness proof

Scenario: watcher 只记录受支持输入
  Test:
    Filter: test_atlas_watcher_records_source_and_manifest_events
    Level: integration
  Given inert watcher 接收 source、manifest、ignored、graph runtime 与 outside-root events
  When event pipeline 写 pending store
  Then 只接受共享 scope 内路径并按 sequence 合并重复事件

Scenario: watch capacity 不伪装 complete
  Test:
    Filter: test_atlas_watch_capacity_reports_partial_or_degraded
    Level: unit
  Given Linux directory cap overflow 与 backend resource error
  When watcher 安装 watch plan
  Then status 明确 partial/degraded、保留手动 build recovery 且不宣称 healthy auto-sync

Scenario: daemon identity 必须通过 live handshake
  Test:
    Filter: test_atlas_daemon_identity_rejects_wrong_root_version_and_nonce
    Level: integration
  Given registry 分别包含 wrong root、wrong graph、wrong version、wrong schema 与 wrong nonce
  When client 连接 loopback endpoint
  Then 每种情况返回 typed identity error 且不发送 sync/stop command

Scenario: daemon singleton 恢复 stale registry 与 PID reuse
  Test:
    Filter: test_atlas_daemon_single_writer_recovers_stale_registry_and_pid_reuse
    Level: integration
  Given daemon lock 空闲但 registry 指向 dead endpoint 或同 PID 的不同 startup nonce
  When 两个 startup attempt 竞争
  Then 一个原子替换 stale record 并服务，另一个只识别该 live owner

Scenario: client disconnect 不停止 daemon
  Test:
    Filter: test_atlas_daemon_client_disconnect_does_not_stop_server
    Level: integration
  Given daemon 已服务一个 status client
  When client socket 关闭且另一 client 随后连接
  Then daemon identity 与 pending state 仍可查询

Scenario: runtime 状态机覆盖所有公开状态
  Test:
    Filter: test_atlas_daemon_reports_warming_pending_degraded_unavailable
    Level: integration
  Given missing graph、initial sync、pending event、retry exhaustion 与 daemon stop
  When status command读取 runtime state
  Then 顺序观察 unavailable、warming、pending、degraded 与 unavailable 且合法成功路径可到 healthy

Scenario: reader lease 阻止旧 generation 提前回收
  Test:
    Filter: test_atlas_reader_lease_preserves_old_generation_until_drop
    Level: integration
  Given query lease 固定旧 generation 后新 generation 已提交
  When writer 执行 safe reclamation
  Then lease 存活时旧 generation 保留且 drop 后下一次 reclamation 删除它

Scenario: ambiguous lease fail closed
  Test:
    Filter: test_atlas_reclamation_retains_generations_for_ambiguous_lease
    Level: integration
  Given locked malformed lease 或无法读取的 lease state
  When writer 扫描 reclaim candidates
  Then 所有旧 generation 保留并返回 maintenance diagnostic

Scenario: abandoned staging cleanup 幂等
  Test:
    Filter: test_atlas_safe_reclamation_cleans_abandoned_staging_idempotently
    Level: integration
  Given writer lock 已持有且存在无 active transaction 的跨进程 staging
  When safe reclamation 连续运行两次
  Then staging 保持不存在且 current/leased generation bytes 不变

Scenario: MCP discovery 与 no-daemon query 保持稳定
  Test:
    Filter: test_atlas_mcp_discovery_is_static_and_no_daemon_queries_match
    Level: integration
  Given unavailable、warming、pending 与 degraded runtime 状态
  When MCP tools/list 和 frozen Atlas query 在 daemon/no-daemon 条件运行
  Then tool list byte-stable、未连接 daemon、graph facts 相同且 live state 显式

Scenario: D3 acceptance matrix 输出完整 receipt
  Test:
    Filter: test_atlas_live_runtime_acceptance_matrix_records_receipts
    Level: integration
  Given checked-in event、retry、identity、supervision 与 reclamation matrix
  When evaluator 驱动 production live-runtime state machine
  Then receipt 包含 watermark、pending、retry、state、generation、writer 与 reclamation 字段

Scenario: D3 docs 与 wiki 保留 derived-authority 边界
  Test:
    Filter: test_atlas_live_runtime_docs_match_cli_and_authority_boundaries
    Level: integration
  Given reader docs、roadmap、skill、CLI help 与 tracked wiki
  When 文档契约测试读取其 watcher、watermark、daemon、degraded、lease 与 no-daemon terms
  Then 页面只描述已存在命令并明确 live runtime 不替代 graph freshness、KLL 或 lifecycle authority
