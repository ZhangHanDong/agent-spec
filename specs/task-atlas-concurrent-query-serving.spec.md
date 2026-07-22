spec: task
name: "Atlas Concurrent Query Serving"
tags: [atlas, daemon, query, concurrency, backpressure, dogfood]
satisfies: [REQ-ATLAS-CONCURRENT-QUERY-SERVING]
depends: [task-atlas-live-runtime, task-atlas-query-context-compiler, task-atlas-query-quality-regression]
estimate: 6d
---

## Intent

在 D3 immutable generation 和 B5 context compiler 上增加可选、固定规模的并发查询服务，
让慢 traversal/source projection 不阻塞 daemon control 或 MCP transport。服务必须在入队前固定
generation、显式拒绝过载，并把 deadline、cancel、panic retry、circuit 与 fallback 留在可机读证据中。

<!-- lint-ack: bdd-rule-grouping - admission、snapshot、worker failure、transport 与 receipt 构成同一个 query-serving 状态机 -->

## Decisions

- 默认 `query_workers=0`，CLI、MCP 与 CI 继续 direct；显式 opt-in profile 使用 2 workers，硬上限 4。
- 使用固定 `std::thread` workers、`sync_channel` bounded queue 和 completion channel；禁止 per-request
  thread、host-core 自动 sizing 和 Tokio global blocking-pool 隐式排队。
- prototype 默认 `queue_capacity=4`、`queue_timeout_ms=2000`、`deadline_ms=20000`、
  `memory_budget_bytes=268435456`、`retry_after_ms=100`；配置必须落在设计文档固定范围。
- listener 在 admission 时创建 `PinnedContextSnapshot`。queued、running、retry 与 response
  serialization 共用同一个 reader lease；worker 只做 frozen read，不重新解析 `CURRENT.json`。
- query execution 在 index load、retrieval 后、projection candidate/source loop 和 serialization 前检查
  cancellation/deadline。queued timeout 在 runner 启动前结束。
- worker panic 用 `catch_unwind` 隔离；第一次只重试一次且沿用 pinned snapshot，第二次打开 circuit，
  circuit 只由 daemon restart 清除。
- protocol 升级为 `agent-spec/atlas-daemon-protocol-v2`。query outcome 固定为
  `success|busy|timeout|cancelled|degraded|failed|unavailable`，只有 success 携带 context。
- manual/auto sync 进入一个固定 maintenance lane，使 status/stop 在 writer publish 期间仍可响应。
- `atlas context` 默认 direct；`--execution worker` 显式走 daemon，`--fallback-direct` 只在 worker
  模式合法且必须记录 worker/fallback 两个 generation，禁止合并 partial evidence。
- `atlas_context` MCP tool 仅在 `AGENT_SPEC_MCP_ATLAS_CONTEXT=1` 时发现；并发 worker route 还要求
  `AGENT_SPEC_MCP_ATLAS_QUERY_MODE=worker`。默认 discovery 与 serial direct server 不变。
- D4 receipt schema 固定为 `agent-spec/atlas-eval/concurrent-query-v1`。语义、snapshot、bounds、typed
  outcome 和 lease cleanup 是 deterministic gate；latency、heartbeat、CPU 与 RSS 只是 measurement。

## Boundaries

### Allowed Changes
- crates/rust-atlas/src/context.rs
- crates/rust-atlas/src/generation.rs
- crates/rust-atlas/src/lib.rs
- crates/rust-atlas/src/locking.rs
- src/atlas_query_service.rs
- src/atlas_daemon.rs
- src/atlas_eval.rs
- src/main.rs
- src/spec_mcp/**
- fixtures/atlas/concurrent-query/**
- benchmarks/atlas/**
- scripts/atlas-eval/**
- docs/atlas-concurrent-query-serving.md
- docs/atlas-live-runtime.md
- docs/atlas-query-context.md
- docs/atlas-evaluation.md
- docs/atlas-roadmap.md
- docs/superpowers/specs/2026-07-21-atlas-concurrent-query-serving-design.md
- docs/superpowers/plans/2026-07-21-atlas-concurrent-query-serving.md
- knowledge/requirements/req-atlas-concurrent-query-serving.md
- specs/task-atlas-concurrent-query-serving.spec.md
- .agent-spec/wiki/**
- README.md
- AGENTS.md
- skills/agent-spec-tool-first/**
- CHANGELOG.md

### Symbols
- rust-atlas: rust_atlas::context::compile_context
- rust-atlas: agent_spec::atlas_query_service

### Forbidden
- 不默认启用 worker 或新增默认 MCP tool
- 不创建 per-request thread、unbounded queue 或 host-core-derived worker count
- 不让 worker refresh/build graph、写 source/KLL 或连接 loopback 以外地址
- 不在 retry、fallback 或 writer publish 时混合两个 generation 的证据
- 不把 busy、timeout、cancelled、degraded 或 unavailable 包装为空成功
- 不把 latency、RSS 或 throughput measurement 当 correctness gate
- 不新增 crate dependency

## Out of Scope

- 远程 query service、跨机器 worker 或多主 daemon
- subprocess crash isolation 与 force-kill Rust thread
- 默认 MCP surface 晋升或自动 direct-to-worker fallback
- A5 framework pack、F1 provider kit 或 F2 non-Rust provider
- 用 D4 benchmark 代替 E1 真实 Agent A/B 与人工接受

## Completion Criteria

Scenario: direct 默认与配置边界稳定
  Test:
    Filter: test_atlas_query_service_defaults_direct_and_validates_bounds
    Level: unit
  Given omitted settings、workers 0、opt-in profile 与每个越界数值
  When QueryServiceConfig 被构造
  Then 默认禁用 worker、opt-in 使用固定数值且越界返回字段化 config error

Scenario: worker 与 direct 返回相同语义
  Test:
    Filter: test_atlas_query_service_worker_matches_frozen_direct_result
    Level: integration
  Given fresh generation 和四种 B5 context profile
  When direct 与 worker 分别执行相同 query
  Then canonical ContextResult bytes、graph fingerprint 和 diagnostics 完全相同

Scenario: full queue 返回 typed busy
  Test:
    Filter: test_atlas_query_service_rejects_full_queue_with_typed_busy
    Level: unit
  Given workers 全部被 barrier 阻塞且 queue 已有 4 个 jobs
  When 第 5 个 queued job admission
  Then 立即返回 atlas-query-busy、retry_after_ms 100 且 runner 未执行该 job

Scenario: memory reservation 不能超预算
  Test:
    Filter: test_atlas_query_service_rejects_memory_reservation_over_budget
    Level: unit
  Given 两个单独合法的 pinned request 合计 reservation 超过配置 memory budget
  When 第一个 running request 尚未完成 response serialization 时提交第二个
  Then 第二个返回 typed busy、只有第一个进入 runner 且 reservation 在 reply drop 后才释放

Scenario: queue timeout 不启动 runner
  Test:
    Filter: test_atlas_query_service_expires_queued_job_before_execution
    Level: unit
  Given fake clock 已越过 2000ms queue deadline
  When worker 取出 queued job
  Then outcome 是 timeout、attempts 为 0 且 QueryRunner 没有调用记录

Scenario: queued cancellation 不启动 runner
  Test:
    Filter: test_atlas_query_service_cancels_queued_job_before_execution
    Level: unit
  Given worker 被 running request 阻塞且第二个 request 已排队
  When 相同 request id 在 worker 取出前被取消
  Then queued outcome 是 cancelled、attempts 为 0 且 runner 调用数不增加

Scenario: executing deadline 在 checkpoint 生效
  Test:
    Filter: test_atlas_query_service_times_out_executing_runner_at_checkpoint
    Level: unit
  Given running worker 持续经过 cooperative checkpoints 且 deadline_ms 是 100
  When execution deadline 到达
  Then outcome 是 timeout、attempts 为 1、context 缺失且 runner 没有继续重试

Scenario: executing cancellation 在 checkpoint 生效
  Test:
    Filter: test_atlas_query_service_cancels_running_projection_and_releases_lease
    Level: integration
  Given runner 已完成 retrieval 并阻塞在 projection checkpoint
  When cancel command 使用相同 request id
  Then outcome 是 cancelled、context 缺失且 response serialization 或 reply drop 后 reader lease 被删除

Scenario: 慢 query 不阻塞 control heartbeat
  Test:
    Filter: test_atlas_query_service_keeps_control_heartbeat_live_during_slow_query
    Level: integration
  Given 两个 running slow queries 和四个 queued queries
  When daemon status、service-status 与 stop 连接到达
  Then listener 在 query completion 前响应且 query queue 状态保持可见

Scenario: pinned generation 跨 writer publish 保持一致
  Test:
    Filter: test_atlas_query_worker_pins_generation_across_writer_publish
    Level: integration
  Given query 在 generation A admission 后阻塞
  When writer 提交 generation B 并执行 safe reclamation
  Then query 只返回 A fingerprint、A lease 存活时不回收且 completion 后可回收

Scenario: panic 只重试一次并打开 circuit
  Test:
    Filter: test_atlas_query_worker_panic_retries_once_then_opens_circuit
    Level: unit
  Given injected runner 连续 panic
  When 同一 query 执行初次 attempt 与一次 retry
  Then attempts 为 2、outcome degraded、circuit open 且下一请求不进入 queue

Scenario: unavailable worker 与 fallback 不混合证据
  Test:
    Filter: test_atlas_query_worker_fallback_records_distinct_complete_generation
    Level: integration
  Given daemon unavailable 或 circuit open 且显式 fallback-direct
  When caller 重新执行完整 direct query
  Then receipt 分别记录 worker 与 fallback generation、只返回 fallback context 且无 partial merge

Scenario: outcome schema 不允许 success-shaped error
  Test:
    Filter: test_atlas_query_service_outcomes_are_typed_and_mutually_exclusive
    Level: unit
  Given success、busy、timeout、cancelled、degraded、failed 与 unavailable responses
  When strict protocol v2 deserializer 检查每种形状
  Then 只有 success 有 context、busy/degraded 有 retry_after_ms 且未知字段被拒绝

Scenario: oversize response 失败而非截断
  Test:
    Filter: test_atlas_query_service_rejects_oversize_protocol_response
    Level: unit
  Given finalized wrapper 超过 1 MiB protocol ceiling
  When daemon 写 response
  Then 返回 atlas-query-response-too-large、释放 lease 且不写 partial JSON line

Scenario: daemon stop 取消并清空 worker jobs
  Test:
    Filter: test_atlas_daemon_stop_cancels_queued_queries_and_drains_workers
    Level: integration
  Given running 与 queued jobs、pending response sockets 和 active reader leases
  When verified owner 发送 stop
  Then queued/running jobs 得到 unavailable 或 cancelled、registry 删除且所有 cooperative workers/leases 清理

Scenario: writer publish 不阻塞 status
  Test:
    Filter: test_atlas_daemon_maintenance_lane_keeps_status_live_during_sync
    Level: integration
  Given manual sync 在 writer commit barrier 等待
  When status 与 stop 请求到达 listener
  Then status 在 sync completion 前返回且 stop cancellation 不清除 pending events

Scenario: MCP context 默认隐藏且 worker transport 可并发
  Test:
    Filter: test_atlas_mcp_context_worker_mode_preserves_discovery_and_ping_liveness
    Level: integration
  Given default env、context-only env 与 worker-route env
  When tools/list、slow atlas_context 和 ping 通过同一 stdio server
  Then 默认列表不变、context 只显式出现且 ping response 可先于 slow context response

Scenario: 两个 worktree 不共享 snapshot 或 counters
  Test:
    Filter: test_atlas_query_service_isolates_concurrent_worktrees
    Level: integration
  Given 同一 repository 的两个 worktree、graph roots 和 daemon identities
  When 两边并发执行同名 query
  Then generation、fingerprint、queue counters、cancel id 与 receipts 不交叉

Scenario: D4 fixture receipt 覆盖完整矩阵
  Test:
    Filter: test_atlas_concurrent_query_checked_in_receipt_is_passing
    Level: integration
  Given versioned direct/worker fixture receipt
  When scorer 验证 semantic parity、typed outcomes、snapshot、lease 与 failed-run retention
  Then correctness gate 通过且 latency、heartbeat、CPU、RSS 只作为 measurement 保存
