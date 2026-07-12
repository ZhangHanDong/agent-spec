# Rust Atlas — Rust 项目静态分析库设计

> Status: approved 2026-07-11（brainstorming 会话确认）
> 背景：受 zerolang "图即程序数据库" 思路启发，但结论是：在 Rust 上这层能力可以用
> stable 工具链零成本自建，不需要新语言。agent 查结构化图代替猜文本 grep。

## 目标

在 agent-spec 仓库中新增独立库 crate `rust-atlas`：扫描任意 Rust 项目，产出可查询、
可增量失效的项目图（符号、模块树、impl 关系、引用边），供 agent 通过库 API、
`agent-spec atlas` CLI 与 MCP 只读工具消费。

## 已确认的关键决策（brainstorming 结论）

1. **深度分两层，schema 统一**：Phase 1 语法+符号层（stable 工具链）；Phase 2 MIR
   增强层（nightly，可选 feature）。同一 schema 通过边上的 `provenance` 字段
   （`syn` | `scip` | `mir`）兼容两层，MIR 只是叠加更高置信度的边。
2. **消费界面**：库 API + CLI + MCP 三个都做（本期）；KLL 集成推后到 Phase 3。
   顺序理由：MCP 是库查询 API 的薄协议壳，必须后于库存在；CLI 零配置可达性最广。
3. **存储与失效**：`.agent-spec/graph/` 下按源文件分片的 JSON + blake3 内容哈希。
   查询前比对哈希，少量脏文件自动增量重建，大量时结果携带 `stale` 警告字段。
   这对应 zerolang "失效哈希在写入前拦截" 要解决的同一问题。
4. **解析后端**：syn 自解析为基座（自包含、零外部工具依赖）；检测到 rust-analyzer
   时可叠加 SCIP 索引获得精确跨文件引用边；缺席时优雅降级并在 meta 标注 capability。

## 在意图编译器中的位置

Rust Atlas 是语言专用的 Code Graph Provider，不是 Requirement IR 前端，也不负责解释或
修改 KLL 需求。它把 Rust 源码编译成派生、可失效的 Code Graph IR；计划中的
Code Grounding / Intent-Code Linker 在 work-unit lowering 之后消费这张图，将 leaf work
unit 绑定到 crate、module、trait、type 和 function，再由 Task Contract 固化人工确认的
Boundaries 与 Symbols。

```text
Accepted Requirement IR -> Work Units --+
                                        +-> Intent-Code Linker -> Task Contract / Plan DAG
Rust Source -> Rust Atlas -> Code Graph -+
```

同一份图随后可被 lifecycle、trace、wiki 和 MCP 只读复用。图事实始终是派生数据，
stale 图不能产生确定性绑定，也不能被提升为长期 KLL 真相。完整的双 IR、治理门禁和
质量工具架构见 `docs/intent-compiler/architecture.md`。

Clippy、rustfmt、cargo-deny、Miri 等不属于 Atlas。它们是 Diagnostic、Transformation
或 Verification Provider，由计划中的 Quality Planning 阶段在 code grounding 之后按
风险等级和项目 profile 选择，并与必需 skills 一起形成 Execution Bundle。Skills 指导
AI 生成代码；工具和 lifecycle 输出才是验收证据。

## 架构

```
crates/rust-atlas (lib, 零 agent-spec 依赖，可独立发布)
  ├── extract/syn_layer    syn 解析 → 节点 + 声明级边
  ├── extract/scip_layer   SCIP 索引摄取 → 解析级 References 边（可选）
  ├── model                节点/边/provenance/meta schema（schema_version）
  ├── store                .agent-spec/graph/ 分片读写 + blake3 哈希
  └── query                tree / query / refs / impls / staleness

agent-spec (bin)
  ├── main.rs              `atlas` 子命令（注意：`graph` 已被 spec 依赖 DAG 占用）
  └── spec_mcp/tools.rs    atlas_tree / atlas_query / atlas_refs / atlas_impls / atlas_status
```

### Schema 要点

- 节点：Crate / Module / Struct / Enum / Trait / Fn / Impl / TypeAlias / Const / Macro；
  字段：规范符号路径 ID、kind、文件、span、可见性、签名摘要、doc 首行。
- 边：Contains / ImplsTrait / ImplFor / References / Calls / UsesType，每条边带
  `provenance`。Phase 1 的 Calls 只有 SCIP 精度；syn 层只产出 References 近似。
- 解析失败的文件记为 `unparsed` 诊断节点，不阻塞全图。

### CLI 面（全部支持 JSON 输出）

```
agent-spec atlas build [--full] [--scip <index>]
agent-spec atlas tree [path]
agent-spec atlas query <symbol>
agent-spec atlas refs <symbol>
agent-spec atlas impls <trait|type>
agent-spec atlas check          # 新鲜度，CI 可用退出码
```

## 分期 Roadmap

| Phase | 内容 | 工具链 |
|-------|------|--------|
| 1（本期） | workspace 转换、rust-atlas 核心、CLI、SCIP 可选增强、MCP 工具 | stable |
| 2 | MIR 增强器（优先评估 Charon，其次 rustc_public 自写 driver），精确 Calls/CFG 摘要 | nightly feature |
| 3 | Intent-Code Linker 首个切片：spec symbol boundary、lifecycle/trace 引用图节点，校验符号存在性与图新鲜度 | stable |

完整的 provider-neutral binding schema、planning-time grounding 与 Quality Planning
分别进入后续独立合约，避免 Atlas 专用接口反向污染通用意图编译器。

## 相关文件

- 合约：`specs/task-rust-atlas-code-graph.spec.md`
- 需求：`knowledge/requirements/req-rust-atlas.md`（人类确认的正典；YAML 兼容投影待导出器实现后派生）
- 实施计划：`docs/superpowers/plans/2026-07-11-rust-atlas-code-graph.md`
- Roadmap specs：`specs/roadmap/task-atlas-mir-layer.spec.md`、
  `specs/roadmap/task-atlas-kll-integration.spec.md`
