spec: task
name: "External Code Graph Provider Adapter Kit"
tags: [atlas, provider, adapter, conformance, dogfood]
satisfies: [REQ-CODE-GRAPH-PROVIDER-KIT]
depends: [task-code-graph-ir-bindings, task-intent-aware-affected, task-atlas-worktree-layered-freshness]
estimate: 5d
---

## Intent

交付 F1 外部 Code Graph Provider Kit，使未来 SCIP、tree-sitter 或本地分析工具可以通过
同一套 Rust 类型、受限进程协议和 conformance gate 投影 provider-neutral Code Graph IR，
同时保持 agent-spec 默认不启动、不安装、不信任任何第三方 provider。

<!-- lint-ack: bdd-rule-grouping — manifest、projection、runner 和 conformance 是同一 F1 adapter kit 的不可拆分发布门 -->

## Decisions

- 新 workspace crate `crates/code-graph-provider` 提供公开 typed SDK；package 名为
  `agent-spec-code-graph-provider`，不依赖 `rust-atlas` 或 agent-spec binary。
- manifest、project registration、request、extraction payload、enrichment payload、published
  artifact 和 conformance receipt 使用独立的 `agent-spec/code-graph-provider/*-v1` schema id，
  并用 `deny_unknown_fields` 拒绝静默 schema 漂移。
- manifest role 固定为 `extractor` 或 `semantic-enricher`。extractor 输出 node、containment 和
  basic-reference；enricher schema 不含 node/KLL 字段，只含 base graph fingerprint、edge、
  query hint、evidence 和 confidence。
- project registration 默认 `enabled: false`；启用时提供一个 executable、literal argv 和可选
  cwd。执行使用 `std::process::Command`，不经过 shell、不下载、不发现 installer、不连接 daemon。
- host 校验 provider/worktree/schema/path/id/order/freshness 后计算 canonical BLAKE3 fingerprint，
  再把 artifact 写入目标同目录临时文件并 rename；失败不改旧 artifact。
- 同步 runner 使用 polling、共享 cancellation token 和独立 bounded stdout/stderr reader；超时、
  取消和输出超限均杀死并回收 child，返回 stable diagnostic code。
- conformance fixture 是协议测试，不是 F2 provider；它覆盖 roadmap 要求的八类不变量，CLI
  `agent-spec atlas provider validate|conformance` 默认只处理本地显式输入。

## Boundaries

### Allowed Changes
- Cargo.toml
- Cargo.lock
- crates/code-graph-provider/**
- fixtures/code-graph-provider/**
- src/main.rs
- docs/code-graph-provider-kit.md
- docs/atlas-roadmap.md
- knowledge/requirements/req-code-graph-provider-kit.md
- specs/task-code-graph-provider-kit.spec.md
- docs/superpowers/specs/2026-07-21-code-graph-provider-kit-design.md
- docs/superpowers/plans/2026-07-21-code-graph-provider-kit.md
- README.md
- AGENTS.md
- skills/agent-spec-tool-first/**
- .agent-spec/wiki/**
- CHANGELOG.md

### Symbols
- agent-spec-code-graph-provider: agent_spec_code_graph_provider

### Forbidden
- 不把外部 provider 实现放进 rust-atlas
- 不使用 shell command string、eval、隐式 daemon、网络发现或 installer
- 不允许 enricher 修改 node、KLL、requirement 或 authoritative extractor fact
- 不在 validation 完成前替换已发布 artifact
- 不把 conformance fixture 描述成真实语言支持

## Out of Scope

- F2 的 SCIP、tree-sitter 或特定语言 provider 实现
- 自动加载 `.agent-spec/providers.json` 到 requirements bind
- provider 包分发、签名、下载、认证、网络 transport 或 daemon lifecycle
- framework semantic pack 和 A5 默认行为

## Completion Criteria

场景: SDK 保持独立 provider boundary
  测试: test_provider_sdk_stays_rust_atlas_independent
  假设 code-graph-provider crate 作为外部 adapter 作者的公共依赖
  当检查 workspace package dependency
  那么 crate 不依赖 rust-atlas 或 agent-spec binary

场景: manifest 固化 provider 能力和运行边界
  测试: test_manifest_validates_role_schema_capabilities_and_limits
  假设 extractor 或 semantic-enricher manifest 声明 schema range、capability、freshness 和 limits
  当执行 strict validation
  那么 合法组合通过且角色不允许的 capability 返回 provider-manifest-capability

场景: provider 必须由项目显式启用
  测试: test_registration_is_opt_in_and_uses_literal_argv
  假设 registration 缺失、disabled 或包含空 executable
  当解析执行配置
  那么 provider 不运行且返回 provider-disabled 或 provider-registration

场景: extraction projection 生成稳定 fingerprint
  测试:
    过滤: test_extraction_projection_is_stable_and_provider_scoped
    层级: integration
  假设 两次 byte-different 但事实相同且 canonical ordering 的 extraction payload
  当校验并投影 artifact
  那么 node id、path 和 graph fingerprint 稳定且 provider/worktree 匹配

场景: 非规范 node id 和 path 被拒绝
  测试:
    过滤: test_extraction_rejects_unscoped_ids_and_unsafe_paths
    层级: integration
  假设 node id 不含 provider prefix 或 path 为绝对、反斜杠、dot segment
  当校验 extraction payload
  那么 返回 provider-node-id 或 provider-path diagnostic

场景: partial 和 stale freshness 不被提升
  测试:
    过滤: test_projection_preserves_partial_and_stale_diagnostics
    层级: integration
  假设 payload 声明 partial 或 stale
  当投影 artifact
  那么 affected paths 与 diagnostic 被保留且 artifact 不宣称 fresh

场景: worktree mismatch 被拒绝
  测试: test_projection_rejects_wrong_worktree
  假设 response worktree id 与 request 不同
  当校验 response
  那么 返回 provider-worktree-mismatch diagnostic

场景: enricher 只能增加可解释语义
  测试: test_enricher_schema_is_additive_and_evidence_bearing
  假设 semantic enricher 返回 edge 或 query hint
  当校验 enrichment payload
  那么 每项含 extractor、evidence、confidence 和 base graph fingerprint

场景: unknown schema 被拒绝
  测试: test_adapter_rejects_unknown_wire_schema
  假设 process 返回未知或旧的 payload schema
  当 adapter 解析 stdout
  那么 返回 provider-schema diagnostic 且旧 artifact 不变

场景: stdout 和 stderr 均受界
  测试: test_process_adapter_enforces_output_limits
  假设 provider 写出超过 manifest 限制的 stdout 或 stderr
  当执行 provider
  那么 child 被回收且返回 provider-output-limit diagnostic

场景: timeout 和 cancellation 可停止 provider
  测试:
    过滤: test_process_adapter_honors_timeout_and_cancellation
    层级: integration
  假设 provider 阻塞超过 timeout 或 cancellation token 被触发
  当执行 provider
  那么 child 被终止且分别返回 provider-timeout 或 provider-cancelled

场景: 发布失败保持旧 artifact
  测试: test_atomic_publish_preserves_previous_artifact_on_failure
  假设目标已有有效 artifact 且下一次 response 非法
  当 run-and-publish 执行
  那么目标 bytes 不变且同目录临时文件被清理

场景: conformance fixture 覆盖 F1 矩阵
  测试: test_provider_conformance_fixture_covers_roadmap_matrix
  假设 checked-in extractor fixture 与 manifest
  当运行 provider conformance harness
  那么 stable-id、determinism、partial、stale/worktree、schema、limit、cancel、atomic 全部有结果

场景: CLI 输出稳定且失败非零
  测试: test_atlas_provider_cli_validate_and_conformance
  假设 manifest、registration 和 fixture 输入为显式本地文件
  当执行 atlas provider validate 或 conformance
  那么 stdout 为 strict JSON、文件输出原子且任一 blocked check 令命令非零
