# Rust Atlas Roadmap：从可信 Rust 图到意图感知的代码智能

> 当前正典 roadmap，修订于 2026-07-21。状态基线：`agent-spec` 1.1.0、
> `rust-atlas` 0.2.0、Atlas 查询基线 `44e2f71`。Wave 1 的 E0、A2/B1、D1，Wave 2 的
> B2/B3/B4、A3、A4 trait v1、A4.1、D2、D3、E3，以及 Track C 的 C1/C2/C3 已交付；后续
> track 的实现状态仍以本文件各条目为准。
>
> 本文用能力轨道替代旧的单序列 Phase 编号。历史合约保留原名称以维持 trace 稳定，
> 但其中的 `Phase 2`、`Phase 3` 不再代表当前排期。

Rust Atlas 将 Rust 源码编译为可失效、可重建的 Code Graph；agent-spec 再把这张图与
需求、work unit、Task Contract、测试、trace 证据和质量策略连接起来。目标不只是查询
符号，而是让 Agent 能解释和验证完整链路：

```text
需求
  -> leaf work unit
  -> Task Contract 与 scenario
  -> code binding
  -> Rust 符号与图路径
  -> test selector 与质量门禁
  -> worktree 与 commit 证据
```

## 1. 范围与不可变原则

### 1.1 Rust Atlas 保持 Rust 专用

`rust-atlas` 是 agent-spec 的第一个 Code Graph Provider，不是通用多语言解析器。它应
继续发挥 Cargo metadata、`syn`、rust-analyzer SCIP 和未来 MIR 带来的 Rust 专用精度。

非 Rust 语言通过 provider-neutral Code Graph IR 接入。独立的 tree-sitter provider、
SCIP provider 或第三方工具 adapter 可以实现同一消费合约，但不进入 `rust-atlas` 核心，
也不改变 Requirement IR。

### 1.2 一条图谱系，三层证据

```text
Rust source
  -> syn baseline       provenance=syn   离线、容错、始终可用
  -> SCIP overlay       provenance=scip  名称解析、调用、类型、宏
  -> MIR overlay        provenance=mir   编译器级调用与控制流
```

- syn 基线必须运行在 stable Rust 上；单个文件不可解析时，按文件降级而不是中止全图。
- SCIP 和 MIR 都是可选 overlay。缺失时必须明确报告 capability，但不能阻塞 syn 基线。
- `provenance` 只回答“哪个分析层观察到事实”。置信度、dispatch 类型与 resolution 强度
  是独立维度，不能通过增加第四种 provenance 混在一起。

### 1.3 派生事实永远不是 KLL 真相

图 shard、查询索引和 code binding 都是可重建工作数据。需求和已接受决策仍由
`knowledge/` 持有。陈旧图不能产生确定性 binding、lifecycle 证据、影响分析结论或归档
证明。

### 1.4 新鲜度必须区分证据层和 worktree

每次查询必须能标识 repository root、git worktree、graph fingerprint，并分别报告
syn、SCIP、MIR 的 freshness。syn 已刷新不能让旧 SCIP 或 MIR overlay 看起来也是最新。

### 1.5 不确定性必须可查询

未解析符号、external target、歧义名称、动态分派候选集、被截断路径、provider 不可用
和陈旧证据都是一等结果。它们不能被转换为空成功，也不能被伪装成确定性边。

### 1.6 正典存储可移植，加速层可替换

按源文件拆分的 JSON shard 继续作为可移植、可审计的正典图存储。反向边、搜索和路径
索引是可重建缓存，可以在 profiling 后选择其他表示。重建索引不得修改源码、KLL 或
Task Contract。

### 1.7 Agent 使用效果属于正确性

单元测试与集成测试仍是硬门禁，但 Agent-facing 图功能只有在真实 Agent 评测中证明
答案正确、内容充分，并比反复 Read/Grep 更容易消费，才算完成。

### 1.8 事实、候选边与查询提示不得混级

Atlas 后续语义增强统一分为三档：

1. **Fact**：由 syn、SCIP 或 MIR 直接观察并能定位 evidence 的事实，可进入正典 shard。
2. **Candidate edge**：由显式 opt-in enricher 生成的有限候选集，必须携带
   `unresolved`、confidence、candidate、extractor 和 evidence；确定性 impact 不能把它
   当成唯一 target。
3. **Query hint**：只在某次查询中解释 runtime boundary 或建议下一步，不写回正典图，
   也不能成为 lifecycle、binding 或归档证明。

一个 mechanism 只有在真实 corpus 的正反例中证明边的语义稳定，才能从 query hint 晋升
为 candidate edge；只有 compiler authority 对某一 call site 证明唯一 target 后，才能把
动态调用晋升为 exact fact。这个分层借鉴 CodeGraph 对 dynamic boundary 的查询期暴露方式，
同时保留 Atlas 更严格的 evidence 与治理边界。

## 2. 已交付基线

以下能力已经在生产代码和历史合约中实现。表中“已交付”描述代码基线；本次新增的
`REQ-ATLAS-SCIP-SEMANTIC` 用于修复历史上两个 Task Contract 共同满足
`REQ-INTENT-CODE-LINKER` 的归属冲突，其 liveness 要在下一次 lifecycle/replay 后单独
成为 `Honored`，不能因代码已经存在而直接推断。

| 能力 | 状态 | 证据 |
|---|---|---|
| syn 图与分片存储 | 已交付 | `REQ-RUST-ATLAS`、`specs/task-rust-atlas-code-graph.spec.md` |
| syn 正确性硬化 | 已交付 | workspace 布局、唯一 id、item 覆盖、诚实的 unresolved 边 |
| SCIP 语义 overlay | 已交付 | `REQ-ATLAS-SCIP-SEMANTIC`、`specs/task-atlas-scip-semantic.spec.md` |
| schema-version 门 | 已交付 | `read_meta` 强校验：不匹配即 `SchemaMismatch`，查询路径响亮失败并提示重建，build 降级全量重建（e90fcb5） |
| E0 离线评测基线 | 已交付 | `REQ-ATLAS-AGENT-EVALUATION`、`specs/task-atlas-agent-evaluation.spec.md`、`docs/atlas-evaluation.md` |
| E3 查询质量回归基础闭环 | 已交付 | `REQ-ATLAS-QUERY-QUALITY-REGRESSION`、两层 query corpus、live fixture probe、`atlas benchmark score` |
| A4.1 runtime-boundary query hints | 已交付 | `REQ-ATLAS-RUNTIME-BOUNDARY-HINTS`、fresh-source AST scan、E3 runtime-boundary live probe |
| A2 edge evidence 与 B1 query index/search | 已交付 | `REQ-ATLAS-EDGE-EVIDENCE-INDEX`、`specs/task-atlas-edge-evidence-index.spec.md`、schema v6 |
| D1 worktree identity 与 layered freshness | 已交付 | `REQ-ATLAS-WORKTREE-FRESHNESS`、`specs/task-atlas-worktree-layered-freshness.spec.md` |
| D3 可选 watcher/daemon live runtime | 已交付 | `REQ-ATLAS-LIVE-RUNTIME`、`specs/task-atlas-live-runtime.spec.md`、`docs/atlas-live-runtime.md` |
| D4 并发 query serving | 已交付 opt-in prototype | `REQ-ATLAS-CONCURRENT-QUERY-SERVING`、20-run fixture receipt、`docs/atlas-concurrent-query-serving.md` |
| B5 query context compiler | 已交付 | `REQ-ATLAS-QUERY-CONTEXT-COMPILER`、四种 profile、双层 loss receipt、`docs/atlas-query-context.md` |
| E1 Agent adoption gate | harness 已交付，真实结论 pending | `REQ-ATLAS-AGENT-AB-GATE`、72-run 三臂 plan、独立 serving schema、`docs/atlas-agent-ab-gate.md` |
| provider-neutral Code Graph IR 与 binding | 已交付 | `REQ-CODE-GRAPH-IR`、`specs/task-code-graph-ir-bindings.spec.md` |
| F1 external provider adapter kit | 已交付 | `REQ-CODE-GRAPH-PROVIDER-KIT`、独立 Rust SDK、八项 conformance receipt、`docs/code-graph-provider-kit.md` |
| Contract 符号与 typed trace 集成 | 已交付 | `REQ-INTENT-CODE-LINKER`、`specs/task-atlas-kll-integration.spec.md` |
| Quality Planning 与 Execution Bundle | 已交付 | `REQ-QUALITY-PLANNING`、`specs/task-quality-planning-bundles.spec.md` |
| Intent-aware affected 与 execution bundle | 已交付 | `REQ-INTENT-AWARE-AFFECTED`、`REQ-AFFECTED-EXECUTION-BUNDLE` |
| Affected trace v2 与 failure replay | 已交付 | `REQ-AFFECTED-FAILURE-REPLAY`、`specs/task-affected-failure-replay.spec.md` |

当前图能力包括：

- Cargo-aware workspace 布局和按源文件 blake3 失效。
- stable-toolchain syn 提取和 parse-error 降级。
- 直接读取 rust-analyzer SCIP protobuf。
- 带 provenance 与 resolution 的 `calls`、`uses-type`、`references`、
  `impls-trait`、`impl-for` 边。
- schema v6 edge evidence：site、extractor、dispatch、confidence、candidate 与 evidence。
- derived query index，以及 tree、query、search、refs、impls、status 的 CLI 查询。
- shared bounded traversal，以及 source-safe `explore`、explainable `flow`、reverse
  `impact` 和 changed-file `affected` 查询。
- graph identity 与独立 syn、SCIP、MIR status；worktree mismatch、陈旧 semantic
  authority、schema 或 query-index 不一致都会拒绝确定性消费。
- stale-aware Contract symbol、code binding、lifecycle 检查和 typed trace target。
- graph load 的 schema-version 强校验：旧 schema shard 不静默半读，拒绝并给出
  可执行的 rebuild 提示。

已有语义规模足以支持更强的消费层。在审计过的 grok-build workspace 上，SCIP overlay
约产生 120,000 条 `calls`、84,000 条 `uses-type` 和 415,000 条 `references`。下一阶段
的主要瓶颈已经不只是事实提取，而是检索、遍历、解释和增量服务。

## 3. 当前缺口

| 缺口 | 后果 |
|---|---|
| 尚无生产级非 Rust provider | F1 已交付 manifest、投影、受限执行与 conformance kit；多语言项目仍需按 F2 实现并验证具体 adapter |
| 官方 MIR producer 与 trait 之外的 dynamic-dispatch mechanism 尚未交付 | 已可消费外部 compiler overlay 并推理 trait candidates；仓库尚不能自行提取 MIR，其他运行时分派仍报告缺失能力 |
| Rust framework 语义尚无独立 pack | route、registration、task/channel 等路径仍可能止于 runtime-boundary hint |
| 尚无真实 Agent A/B 执行结果 | E1 三臂/并发 harness 已交付，但 checked-in manifest/plan 不是运行证据，不能证明 Atlas 带来性能改善 |
| D4 worker 与 MCP context 仍为 opt-in | correctness、backpressure 与 transport isolation 已交付；没有 E1 真实并发收益证据，不能默认启用 |
| pinned-repository observation 尚未自动刷新 | E3 已固定真实仓库 revision、golden symbol/path 和失败归因，但 fresh capture 仍是显式外部步骤，默认测试不能证明当前 pinned checkout 的实时输出 |

## 4. 能力轨道

各轨道独立演进。交付物之间的依赖决定顺序，轨道编号不要求无关工作互相等待。

### Track A：Graph Accuracy and Evidence

#### A0. syn 基线与硬化

状态：已交付。

- 正确识别 Cargo workspace ownership 与 module layout。
- 稳定 symbol id 和 schema invariant validation。
- 覆盖当前支持的 Rust declaration 类型。
- 明确区分 `resolved`、`unresolved`、`external` edge state。

#### A1. SCIP 语义 overlay

状态：已交付。

- 直接摄取 rust-analyzer SCIP protobuf。
- 生成 resolved call、reference、type use 和 implementation relation。
- overlay 可逆，不修改 syn 基线事实。
- 持久化 SCIP 路径和 fingerprint，支持增量 re-overlay。

#### A2. Evidence-complete edge schema

状态：Wave 1 已交付（schema v6）。

为每条非 containment edge 增加向后兼容的可选字段：

```text
site          file、start/end line 与 column
extractor     analyzer identity 与 version
dispatch      static、trait、generic、closure、function-pointer、channel、macro
confidence    exact、bounded-candidates、heuristic
candidates    无法证明唯一 target 时的候选集合
evidence      analyzer-specific reason 或 occurrence identifier
```

验收要求：

- schema 版本号不匹配的旧 shard 响亮拒绝并提示重建（沿用 e90fcb5 的
  `SchemaMismatch` 语义——派生数据以重建代替 migration）；`serde(default)`
  仅用于同一 schema 版本内新增可选字段。
- 每条 SCIP call edge 保留 occurrence site。
- 存在多个 candidate 时，dynamic edge 不能标为 exact。
- Edge 去重 identity 包含 source、target、kind 和 call site。
- 查询结果无需打开原始 shard 就能解释每一跳。

交付合约：`task-atlas-edge-evidence-index`。后续 A3/A4 不包含在本次交付中。

#### A3. MIR overlay

状态：Wave 3 已交付 versioned overlay consumer、feature gate、fixed-argv driver adapter、
calls/CFG projection、独立 freshness 与失败降级。官方 `rustc_public` producer binary 尚未
随仓库分发；发布前 `--mir <artifact>` 是可用入口，`--mir-driver` 是 producer process
protocol，不把 fake/test producer 当成 compiler authority。

- Charon 已在 2026-07-20 被兼容性门拒绝；目标 producer 是单独钉住 nightly 的
  `rustc_public` driver，不能进入默认 stable dependency graph。
- 增加精确 MIR call edge 和 per-function CFG summary。
- 外部 producer 独立钉住 nightly/extractor version；Atlas 激活入口必须 feature-gated。
- 默认保留 generic form，不展开所有 monomorphized instance。
- MIR 不可用时降级到 syn 加 SCIP，并返回 typed diagnostic。

MIR 应增强一个已经能够解释 evidence 和 flow 的消费层。因此它依赖 A2 和第一版查询
索引，但不阻塞这些高收益能力先落地。

#### A4. Rust dynamic-dispatch enricher

状态：Wave 3 已交付 trait-method v1；Wave 7 已交付 A4.1 query hint；A4.2 mechanism
enricher plugins 仍是未来工作。

v1 由 `atlas build --dynamic-dispatch` 显式启用，只从 resolved SCIP call 指向 trait method
这一高精度 anchor 出发。它保留 exact declaration edge，并通过 resolved `ImplsTrait` 与
containment edge 增加 `unresolved`、`bounded-candidates`、`dispatch: trait` 的 implementation
候选。候选按 canonical id 排序去重，fan-out 硬上限为 64；超限报告
`dynamic-dispatch-truncated`，不写部分集合。没有 anchor 时 pass 是严格 no-op。

候选机制包括 trait object、closure/function pointer、async task spawn、channel、callback
registry 和选定的 Rust framework route。whole-graph 或 framework 推理必须与 core parser
隔离，并输出 bounded candidate 与显式 confidence。除 trait method v1 外的机制仍需各自的
corpus、inert gate、fan-out policy 与 false-positive 验证。

后续拆成两个互不混淆的交付面：

- **A4.1 Dynamic boundary explanation（已交付）**：当 `flow` 在注册表、channel、callback、反射或
  framework dispatch site 终止时，返回 site、mechanism、候选 continuation 和
  `runtime-boundary` diagnostic。查询按 source-first 顺序扫描 source 及其静态可达函数，因此
  新鲜 SCIP helper edge 不会隐藏 caller 中的 runtime site。它是 query hint，不改 shard，
  不参与确定性 impact。每次扫描先按 node name、signature 与 span 绑定唯一 function AST，避免
  同行 sibling 误归属；签名比较共享 canonicalization，receiver role 只读取实际 receiver chain 的
  AST identifier 并使用 token boundary；候选查找按 source context 规范化 Rust 相对路径，并保留
  qualified-self 的类型、trait、generic arguments 与 member。stale source 不扩展 scan frontier，
  stale SCIP/MIR edge 也不能引入 helper；default trait method 的 lowercase `self`/`super` 按 trait
  declaration module 解析。
- **A4.2 Mechanism enricher plugins**：closure/function pointer 优先消费 MIR；async spawn、
  channel、callback registry 与 framework route 各自拥有独立 extractor id、正反 fixture、
  fan-out 上限和 capability。不能用一个通用“猜测 edge”pass 混合所有 mechanism。

晋升门：同一 mechanism 必须证明默认关闭时严格 inert、启用后不删除更高 provenance
事实、候选顺序确定、超限 fail-closed、false-positive/false-negative 可计量。未过门的
mechanism 只能停留在 A4.1。

#### A5. Rust framework semantic packs

状态：未来工作；按真实 Rust 项目需求逐个交付，不建立一个默认开启的通用规则集。

CodeGraph 证明 framework-aware route、registration 和 lifecycle 语义能补足纯语言图，但也
暴露了按命名和目录猜测 target 的误连风险。Atlas 将每个 Rust framework 作为独立 semantic
pack，而不是继续扩张 core parser 或 A4 的通用 mechanism pass：

- 首批候选仅来自 E0/E3 corpus 中反复出现的 Axum、Actix Web、Tonic、Tokio task/channel
  与 Cargo build-script/generated boundary；没有实际需求和 fixture 的 framework 不排期。
- pack manifest 固定 framework/package version range、detect rule、capability、extractor id、
  resource limit 和默认开关；检测到 Cargo dependency 只表示 pack 可用，不等于边已被证明。
- attribute/procedural macro 展开、typed API 和 Cargo metadata 能唯一定位时，可以输出带
  site/evidence 的 fact；名称、目录和 builder-chain heuristic 只能输出 bounded candidate 或
  query hint。
- 每个 pack 必须有真实仓库正例、同名负例、framework 未安装的 inert control、版本不匹配、
  fan-out overflow 与 stale-source fixture，并在 E3 单独报告 precision/recall。
- framework pack 可以依赖 syn、SCIP 或 MIR capability，但缺失依赖时必须降级并说明，不能
  用更弱 heuristic 冒充原 capability。

第一份 pack 只在某个 framework 的真实 Agent 问题持续止于同一 runtime boundary 后立项。
其合约必须独立于 `rust-atlas` 基线和其他 pack，以便单独禁用、升级和回滚。

### Track B：Agent Query and Retrieval

#### B0. 现有低层查询

状态：已交付。

library、CLI 和 MCP 已提供 tree、query、refs、impls、status；CLI 另提供 indexed
`search`。即使未来默认 Agent surface 收敛，这些稳定 primitive 仍然保留。

#### B1. Search、disambiguation 与 derived query index

状态：Wave 1 已交付（与 A2 同一合约）。

- 为 symbol/name、file-to-node、incoming/outgoing edge by kind 建立可重建索引。
- 支持 exact、qualified、segmented identifier 与 deterministic fuzzy search。
- 返回排序后的 ambiguity candidate、canonical id 与 location。
- JSON shard 仍是正典存储，index recreation 必须 atomic。

当前实现使用 JSON-side derived index；并未交付 SQLite 或其他 embedded index。

#### B2. 综合查询 `atlas explore`

状态：Wave 2 已交付；MCP 入口保持 opt-in。

`atlas explore` 是确定性组合查询，不在 Atlas 内调用 LLM。它从输入中提取 identifier 和
path，查询图后一次返回受预算约束的结果：

- 相关 symbol 与新鲜 source excerpt；
- relationship map 和关键 path spine；
- caller、callee、implementation 与 blast-radius summary；
- 每一跳的 site、provenance、resolution、dispatch、confidence；
- stale、unavailable、ambiguous 与 truncation diagnostic。

输出必须支持至少两种确定性预算：面向路径问题的 compact spine，以及面向架构解释的
bounded deep context。不能因为综合查询存在，就强迫所有问题承担同样的 source payload。
compact 固定为 8 seeds、32 nodes、48 edges、8 paths、4 excerpts、每段 20 行和
16,000 serialized bytes；deep 固定为 16、96、160、20、12、40 和 24,000 bytes。
超限时先按固定顺序裁剪可选 section；如果 status、diagnostic、seed 或主 spine 等必需证据
自身已超过硬 byte cap，则整次查询返回 typed budget error，不产生超限或证据残缺的 JSON。

只有当前源码 hash 与选择它的图层匹配时，才能内联 source excerpt。frozen stale query
不能把旧图路径和未标注的当前源码混在一起。

现有低层 CLI 继续保留。`atlas_explore` 仅在
`AGENT_SPEC_MCP_ATLAS_EXPLORE=1` 时进入 MCP discovery 和 dispatch；是否默认暴露仍必须由
Track E 的真实 Atlas A/B 结果决定，不能仅凭其他项目经验直接修改。

交付合约：`REQ-ATLAS-EXPLORE-FLOW-IMPACT`、`task-atlas-explore-flow-impact`。

#### B3. Flow query

状态：Wave 2 已交付。

```text
atlas flow --from <symbol> --to <symbol>
atlas flow --through <symbol>
```

- 返回有界的 shortest path 和 highest-confidence path。
- dispatch 有歧义时保留 alternative path。
- traversal 前区分 unknown 与 ambiguous endpoint；多个 suffix candidate 不得擅自选取。
- 区分 no-path、capability unavailable 和 search truncated。已存在 syn path 时即使 SCIP
  unavailable 仍返回 found；只有未找到 path 且 SCIP 不可用时才不能宣称 no-path。
- spine 中的完整 `Node` 提供 canonical location、signature 与 doc；需要实际 source excerpt 时由
  `atlas explore` 按逐文件 hash 组合，`atlas flow` 本身不读取整组参与文件。

#### B4. Code impact 与 affected test

状态：Wave 2 已交付 code impact；C1/C2 已将其连接到 Intent-Code Linker、显式 Contract
selector、test obligation 与质量策略。

```text
atlas impact <symbol> --depth <n>
git diff --name-only | atlas affected --stdin
```

- 反向遍历 call、reference、type use、impl 与 containment edge。
- 输入支持 symbol、file、stdin、staged change、worktree change 和 commit range；affected CLI
  每次必须且只能选择一种输入模式，VCS 仅通过固定 Git argv 调用。
- 每个 affected node 返回 path 与 distance，不只返回平铺列表。
- 不得仅凭测试文件名模式断言确定性 test coverage。
- 输出 provider-neutral result，供 Intent-Code Linker 与 test obligation、Contract
  selector 连接。

#### B5. Query context compiler

状态：已交付加性 CLI/library 与 E3 回归；默认 MCP 变化仍依赖 E1。

把“图检索”和“给 Agent 的上下文编排”分成两个确定性阶段。检索阶段返回完整候选与评分
理由；context compiler 再按显式 profile 生成 bounded output：

- `symbol`：精确声明、签名、定位、caller/callee 摘要；
- `flow`：主 spine 的 source body、alternative path 与 runtime boundary；
- `architecture`：关键模块、relationship summary 和少量代表性实现；
- `impact`：reverse path、unresolved frontier、binding 与 test obligation 缺口。

压缩只允许把主 spine 之外、可互换且已有代表实现的 sibling body 降为 signature
skeleton。用户点名的 symbol、唯一实现、boundary site、失败证据和 source span 不得被压缩。
每个结果返回 omission manifest，列明省略原因、数量、预算与可执行的后续查询；不得用
“少一次工具调用”掩盖更大的 payload 或必要 read-back。

context compiler 的内部合约进一步固定为：

```text
QueryIntent
  -> RetrievalCandidateSet + scoring reasons
  -> EvidencePriorityPlan
  -> ContextProjection + OmissionManifest + QueryReceipt
```

- `QueryIntent` 只做确定性的 identifier、path、relation 和显式 profile 解析，不在 Atlas 内
  调用 LLM，也不把自然语言猜测写回图。
- Evidence priority 固定为：用户点名 symbol 与失败证据；主 spine 与 boundary site；唯一或
  representative implementation；相邻结构；off-spine sibling。测试、generated file 和 vendor
  source 只有在问题点名或它们位于证据 spine 时进入正文。
- source projection 优先围绕 symbol span 与 edge site 生成可校验 line slice，而不是按文件
  整体填满预算。relevance threshold 先于 byte cap，预算是上限而不是必须填满的目标。
- `OmissionManifest` 为每类被裁剪内容记录 count、reason、最高分候选与稳定 continuation
  query；后续查询必须能从同一 graph fingerprint 恢复，不能依赖进程内隐藏游标。
- `QueryReceipt` 分开记录 retrieval recall、projection retention、serialized bytes、被截断的
  evidence class、read-back 和 follow-up；这样 E3 能判断问题来自图检索还是上下文编排。

profile、预算和排序 tie-break 必须显式且确定，不能根据隐藏运行时状态改变语义。项目规模
可以给出 profile 建议，但正式 receipt 必须记录实际 profile、limit、serialized bytes、
read-back 和 follow-up query。

已交付实现位于 `crates/rust-atlas/src/context.rs`，入口为
`atlas context <query> --profile symbol|flow|architecture|impact`。continuation 在 retrieval hard
cap 之前按 stable evidence id 分页，并用 graph fingerprint 拒绝跨 generation 恢复。E3
`2026-07-21.1` 固定四 profile、8 KiB projection pressure 与 stale-source receipt；默认测试还会
现场重建 fixture graph。交付观测中普通 profile retrieval 数为 13/23/12/6，pressure case
保留 3/12、裁剪 9 项并输出 7267 bytes。该结果不构成真实 Agent A/B 或默认 MCP 晋升证据。

### Track C：Intent-Aware Impact and Execution

#### C0. Binding 与 lifecycle 集成

状态：已交付。

- ready work unit 可以绑定 fresh provider node。
- Task Contract 可以声明 canonical symbol。
- lifecycle 检查 missing symbol 与 stale graph。
- trace target 记录 provider、node、file、provenance 和 graph fingerprint。

#### C1. Intent-aware `affected`

状态：已交付（在 B4 之后）。

将 code impact subgraph 与 agent-spec 已有工件连接：

```text
changed file or symbol
  -> affected Atlas node and path
  -> code-bindings.json
  -> requirement and leaf work unit
  -> Task Contract and scenario
  -> Test selector or test obligation
  -> quality profile and required skill
  -> worktree and commit evidence
```

machine-readable result 必须列出链路缺口，例如 affected node 没有 binding、scenario 没有
test selector 或 test obligation、worktree manifest/VCS 未观察到，或者 required provider
不可用。不得静默丢弃这些路径。

交付合约：`task-intent-aware-affected`。CLI：`requirements affected`；schema：
`intent-impact-v1.schema.json`。

#### C2. Affected execution bundle

状态：已交付（在 C1 之后）。

- 根据 graph impact 和 requirement risk 为一个 work unit 选择 fast check 与 acceptance
  gate。
- risk A 要求 lifecycle、trace、targeted tests 与 adversarial review；risk B 要求 lifecycle
  与 trace；risk C 只要求 lifecycle。
- 所选 quality provider 保留 executable、argv、cwd、timeout 与 output limit，bundle 不退化
  为不可执行的 provider id 列表。
- 通过显式 Test selector 和 test obligation 选择测试；文件名 heuristic 只能提议候选。
- 从 project guidance 解析 required skill，记录 immutable skill receipt，但不把 receipt
  当作通过证据。
- 解释每个 tool、test、skill 被纳入的原因。

交付合约：`task-affected-execution-bundle`。CLI：`requirements affected-bundle`；schema：
`affected-execution-bundle-v1.schema.json`。

#### C3. Failure explanation 与 replay 增强

状态：已交付（在 C1/C2 之后）。

扩展 failure/replay surface，使一次查询能回答：

```text
哪个 requirement
哪个 leaf work unit
哪个 scenario 与 test
哪个 graph node 与 source span
哪条 path、哪个 worktree 与 commit
哪个 lifecycle 或 quality verdict 失败
```

Replay 仍是对已保存确定性记录的 evidence replay，不是 LLM rerun，也不承诺模型能重新
生成完全相同的代码。

`requirements affected-record` 将已保存 intent-impact、可选 affected bundle 与归一化
quality outcomes 合并进 trace ledger v2；同一 `run_id` 的 lifecycle records 保留在同一
ledger 文件中。重复的 partial record 保留已有 bundle/quality evidence，冲突的 immutable
evidence 响亮拒绝。`requirements replay`、`requirements explain-failure` 和
`requirements trace-graph` 只读取这些记录，不会重跑 Atlas、Git diff、测试、quality
provider、skill 或模型。v1 ledger 继续可读，但返回 `affected-trace-missing` gap。schema：
`requirement-trace-ledger-v2.schema.json`。

### Track D：Live Runtime and Large Workspaces

#### D1. Worktree identity 与 layered freshness

状态：Wave 1 已交付（在 A2/B1 之后）。

- metadata 包含 git common dir、worktree root 与 graph root。
- 检测从其他 worktree 借用的图，并拒绝确定性消费或清晰标注。
- 分别报告 syn、SCIP、MIR freshness。
- fingerprint 包含 analyzer 与 toolchain version。
- binding、lifecycle 和 query gate 消费同一 freshness result。
- schema mismatch 保持优先失败；同 worktree 的 query-index 缺失、schema、fingerprint
  或完整性错误也要求 `atlas build` 重建，而不是返回部分结果。

交付合约：`task-atlas-worktree-layered-freshness`。D2/D3 不包含在本次交付中。

#### D2. 增量 resolution 与 validation

状态：Wave 8 已交付，合约 `REQ-ATLAS-INCREMENTAL-HARDENING` /
`task-atlas-incremental-hardening`。

D2 先交付可证明正确的增量事务，再为 D3 提供同步原语：

- **D2.1 Input plan**：缓存 Cargo metadata，但 cache key 必须包含 workspace manifests、
  toolchain、features、target/cfg 和 provider version；只按 `Cargo.toml` mtime 复用不合格。
  query 触发的 stale refresh 继续使用 committed plan 的 features、target/cfg，不得静默回默认配置。
- **D2.2 Dependency frontier**：changed file 重新提取 declaration 后，重算其直接边以及受
  symbol 增删、module ownership、impl relation 影响的 reverse dependent。frontier 必须有
  上限；超限升级为显式 full rebuild，不能静默漏边。
- **D2.3 Recoverable work queue**：未完成 resolution 保存为可恢复的 orphan work item。
  后续零变更 sync 也要检查并清空 orphan；成功或确定性 unresolved 都消费 item，进程中止
  不能让调用边永久缺失。
- **D2.4 Generation commit**：shards、meta、query index 和 overlay capability 以同一
  generation manifest 发布。reader 固定读取一个 committed generation；cancellation、写满、
  rename 失败或进程崩溃后，旧 generation 仍完整可读；本事务 staging 可幂等清理且不删除
  committed generation。跨进程遗留 staging 与旧 generation 回收等待 D3 retention contract。
- **D2.5 Fast path 与 maintenance**：zero-change rebuild 不运行全图 resolution、validation
  或重写文件；统计、压缩和 cache maintenance 不得把已经完成的 build 变成长尾失败。
- **D2.6 Resource contract**：resolution 和 validation 分批、可取消且有内存上限；取消只
  丢弃未发布 generation，不留下 partial authority。确定性 byte admission 同时覆盖 source、
  serialized shards 与显式 overlay；capability 切换以显式 full frontier 执行并报告 fallback。

验收矩阵覆盖 cold build、zero-change rebuild、single-file declaration edit、删除文件、
workspace manifest edit、frontier overflow、overlay activation、cancellation、generation commit
failure 和 orphan recovery。确定性 receipt 记录 touched shards、resolved/unresolved edge delta、
bounded working bytes、generation id、input-plan result、orphan count 与 fallback reason；耗时和
操作系统 RSS 不作为 correctness gate。

详细执行顺序与故障注入矩阵见
`docs/superpowers/plans/2026-07-20-atlas-d2-incremental-hardening.md`。读者指南见
`docs/atlas-incremental-builds.md`。`accepted` 仍表示治理范围已获确认；实现交付证据由当前
Task Contract 的 lifecycle、fixture matrix、trace/replay 与本文件第 8 节门禁共同给出。

#### D3. 可选 watch 与 daemon mode

状态：已交付（Wave 9）；依赖 D1、D2。

- **D3.1 Bounded watcher**：macOS/Windows 优先单个 recursive watch；Linux 按目录监听并
  设置硬上限。watch scope 与 Atlas build scope 共享同一 ignore/config 解析，不能各自漂移。
- **D3.2 Pending watermark**：每个事件记录 path 与 sequence/time watermark。sync 只清除
  本次快照之前且已成功提交的事件；sync 中到达的新事件、锁冲突和普通失败都必须保留并
  触发下一轮。查询按返回结果涉及的文件与 pending 集求交，给出局部 stale 标记。
- **D3.3 Bounded retry**：锁竞争与 extractor/IO 等普通失败分别计数，指数退避且有上限；
  超限进入 typed degraded 状态并保留 pending，不得无限重试或继续宣称 auto-sync 正常。
- **D3.4 Daemon identity**：daemon 以 canonical worktree root、tool/schema version 和启动
  identity 绑定。并发启动只能有一个 writer；dead pid、PID reuse、stale socket/lock、版本
  不匹配和 worktree 删除重建都必须恢复或拒绝，而不是附着到错误进程。
- **D3.5 Static discovery**：MCP tool discovery 和 help 不等待 graph warm-up；调用结果可以
  返回 `warming`、`pending`、`degraded` 或 `unavailable`。为 CI、sandbox 和确定性运行保留
  显式 no-daemon mode。
- **D3.6 Supervision**：客户端退出不误杀仍有其他客户端的 daemon；daemon 中止时 client
  获得 typed failure。watch/daemon 不写用户的 Agent 配置，也不成为查询正确性的前提。
- **D3.7 Safe reclamation**：只有 single-writer identity 与 reader lease 足以证明路径
  不再被使用时，才回收跨进程遗留 staging 或旧 generation；否则保留并报告 maintenance 状态。

已交付实现使用共享 `AtlasScope`、16 MiB/100000-path pending journal、5 次独立 retry
budget、loopback identity handshake、single writer、跨进程 reader lease 和 fail-closed
reclamation。MCP discovery 保持静态，no-daemon query 与 daemon query 固定相同 generation
事实。Daemon 是正确增量模型之上的优化，不是对低效全图重算的遮蔽；`pending` 或
`degraded` 不替代 graph freshness、KLL 或 lifecycle authority。

#### D4. Concurrent query serving and backpressure

状态：已交付 opt-in prototype（Wave 10）；依赖 D3 snapshot lease 和 B5 load profile。
是否默认启用仍由 E1 的并发负载数据决定。

CodeGraph 的 query worker pool 说明：共享 daemon 即使图已新鲜，CPU-heavy traversal 和 source
projection 仍可能阻塞 MCP transport。Atlas 因此交付了可测量、默认关闭的服务契约：

- MCP transport、daemon control 和 status 查询不得被长 traversal 占用；读查询必须固定到
  一个 generation，不能在 worker 间混合 snapshot。
- bounded queue、worker 数、单查询 deadline、内存预算和 cancellation 都是显式配置，并在
  status/receipt 中可见；禁止无界排队或每请求创建线程。
- worker crash 最多重试一次；重复 crash 触发 circuit breaker，降级到受限的 in-process 或
  no-daemon 查询，并返回 typed `busy`/`degraded` diagnostic 与 `retry_after_ms`。
- 不采用“成功形状的 busy 文本”伪装完成结果。过载、超时、查询失败和图不可用保持可机读
  区分，Agent 才不会把未执行的查询当作空成功。
- 验收覆盖单 client、并发 burst、慢查询、worker cold start/crash、queue timeout、daemon
  stop 与 writer publish；同时验证 transport heartbeat、结果 fingerprint 和 reader lease。

当前实现使用 2-worker/4-queue opt-in profile、固定 maintenance lane、daemon protocol v2、
CLI direct/worker/fallback、隐藏的 MCP `atlas_context`、七种 typed outcome 和严格 D4 receipt。
20-run fixture 覆盖四种 B5 load profile、queue/memory busy、timeout、cancel、panic/circuit、
publish/stop/fallback 与双 worktree 隔离。语义、snapshot、bounds 与 lease cleanup 是 gate；
latency、heartbeat、CPU、RSS 只记录 measurement。

只有 E1 证明并发 Agent/worktree 场景有稳定收益且 correctness 不回退时，worker pool 才默认
启用。单 Agent、小仓库和 CI 继续保留零 worker 的直接路径。

### Track E：Evaluation and Adoption

#### E0. Rust benchmark baseline

状态：Wave 1 离线基线已交付；真实 Agent A/B 尚未执行，默认 MCP surface 仍不变。

建立可复现 corpus，覆盖 small、medium、large Rust workspace。每次能力变更至少测试以下
问题类型：

- symbol 与 implementation discovery；
- request/event flow reconstruction；
- change impact 与 affected test；
- 需要编辑与验证的 implementation task；
- stale、SCIP unavailable、compile-failing、alternate-worktree 场景。

#### E1. Agent A/B gate

状态：严格三臂 Agent 与 direct/worker harness 已交付；真实执行仍为 opt-in，尚无 receipt、人工接受或通过结论。

- 使用相同 model、prompt、repository revision、permission、tool instructions 和 cold/warm
  condition；环境中已有的 prompt hook、MCP 配置与用户级 skill 必须在各 arm 对称或显式禁用。
- 使用三臂而不是只做 with/without：A 为 built-in Read/Grep；B 为当前 Atlas primitive/
  `explore` 基线；C 为 B5 context compiler 候选。B 对 A 证明 Atlas 的总价值，C 对 B 隔离
  context compiler 的增量价值。
- 每个 arm 至少运行三次，报告 median 与 variance。
- 先由版本化 ground truth/rubric 或盲评判断 answer correctness，再测 file read、grep、graph
  call、total tool call、round trip、wall-clock、response bytes、context size 与 cost。
- 实验 manifest、arm 配置、parser、judge version 和失败 run 必须进版本化 receipt；原始 session
  可以外置，但其 hash 与保留位置必须记录。正式矩阵不能依赖 `/tmp` 中未提交的 canonical
  driver，也不能删除失败 run 后只统计成功样本。
- query metric receipt 必须携带版本和完整字段；legacy receipt 单独计数且不得作为零值样本改善
  A/B 指标，正式 gate 要求两组 legacy count 均为零。
- 不允许 correctness regression，也不允许把 stale result 展示为 fresh。
- medium/large repo 应显著减少 Read/Grep、round trip 和总 tool call；small repo 允许进入明确的
  tie zone，但不能隐藏启动与 payload overhead。具体阈值来自 E0，不复制其他项目的 benchmark
  数字。
- 默认 MCP surface、B5 profile 或 D4 worker 默认值只能依据对应 question class 的结果调整；
  架构问题上的收益不能替代 implementation、impact、stale 与 failure-replay 场景的验证。
- D4 另做 direct 与 worker 两臂的并发 burst 实验；单请求 A/B/C 的吞吐数字不能替代 transport
  heartbeat、tail latency、queue timeout 和 snapshot correctness。

#### E2. Coverage 与 honesty metric

状态：持续执行。

按 workspace 与 provenance layer 报告：

- resolved、unresolved、external、ambiguous edge；
- 有 resolved cross-file dependent 的 file 与 symbol；
- exact path 与 bounded-candidate path；
- fixture 中的 false positive 与 false negative；
- `atlas explore` 后的 read-back；
- query truncation 与 fallback rate。

#### E3. Query quality regression loop

状态：基础闭环已交付；fresh pinned-repository capture 与 E1 Agent A/B 持续执行。

建立两层、版本化的 query corpus：

1. **小型确定性 fixture**：覆盖 parser、resolution、排序、预算、stale 和 negative path。
2. **固定真实 Rust 仓库 revision**：每个问题保存 expected symbols、expected/forbidden path、
   required evidence、允许的 ambiguity 与答案 rubric。

每次 ranking、traversal、dynamic boundary 或 context projection 变化都产出 machine-readable
receipt，至少包含 recall、MRR、path precision/recall、forbidden-hit、response bytes、latency、
read-back、follow-up query 和 capability/stale diagnostic。单纯“结果里出现过目标符号”不是
通过；主路径错误、隐藏 stale 或省略关键 boundary 都是 correctness failure。

生产问题进入固定闭环：最小复现 fixture -> 真实仓库 case -> 修复 -> corpus regression ->
必要时 Agent A/B。新增 language/provider 不能只凭主观“LLM 看起来够用”宣布支持。

真实仓库层必须保存可重建的 capture manifest：repository URL、commit、subdir、build features、
Atlas/provider version、query、expected/forbidden evidence 和采集命令。checked-in observation
需要能通过同一 scorer 重放；fresh capture 可以是显式网络步骤，但其 harness 和 parser 必须
在仓库中，不能只把一次性 matrix driver 留在临时目录。

当前交付物为 `agent-spec/atlas-eval/query-corpus-v1` 两层 corpus、严格
`query-results-v1` observation、fingerprinted `query-regression-v1` receipt，以及
`agent-spec atlas benchmark score`。默认测试离线重建 `fixtures/atlas/basic`，把当前
`rust_atlas::search` 与 `rust_atlas::flow` 输出送入同一 scorer；错误 path、forbidden hit、
缺失 evidence/diagnostic 或超出 ambiguity allowance 都会写入 receipt 后令 CLI 非零退出。
真实仓库 case 固定到 agent-spec commit `ac381949e13e2f3b0fe0aad6aa7bb06bb8dde1d2`，
但默认测试只评分 checked-in observation，不 clone、fetch 或执行该 revision。

### Track F：Provider Ecosystem

#### F0. Provider-neutral consumer contract

状态：已通过 `REQ-CODE-GRAPH-IR` 交付。

#### F1. External provider adapter kit

状态：已通过 `REQ-CODE-GRAPH-PROVIDER-KIT` 交付。

- 定义 `ProviderManifest`：provider id/version、language、schema range、capability、启动方式、
  freshness inputs、resource limit 与 deterministic/no-daemon 支持。
- 分离 extraction provider 和 semantic enricher。前者投影 node、containment 与基础 reference；
  后者只能增加带 extractor/evidence/confidence 的 edge 或 query hint，不能修改 KLL。
- 文档化 node/edge projection、freshness、graph fingerprint、path normalization、diagnostic 与
  error normalization。
- 增加 provider-neutral conformance fixture，覆盖 stable id、重复构建确定性、partial parse、
  stale/worktree、unknown schema、bounded output、cancellation 和 atomic publish。
- adapter 必须可选且由项目配置；agent-spec 不绑定单一供应商、runtime、installer 或
  orchestration system。CodeGraph adapter 可以是 F2 候选，但没有特殊协议地位。

#### F2. 非 Rust provider

状态：需求驱动。

候选包括 generic SCIP adapter、独立 tree-sitter provider，或已有本地 Code Graph 工具的
adapter。它们投影到同一 Code Graph IR，并通过 provider conformance test，但不成为
`rust-atlas` 内部模块。

## 5. 从 codegraph 吸收的经验

本轮基于本地 checkout 审查了
[codegraph](https://github.com/colbymchenry/codegraph) `v1.3.1`
（commit `e552dc2`）。参考边界是源码中已经有测试的机制，而不是 README 的产品宣称：

| 审查面 | codegraph 证据位置 | Atlas 对应轨道 |
|---|---|---|
| changed-file sync、orphan recovery、write lock | `src/index.ts`、`src/db/**`、`__tests__/sync.test.ts` | D2 |
| pending watermark、bounded watch/retry/degrade | `src/sync/watcher.ts`、`__tests__/watcher.test.ts` | D3 |
| daemon single-writer、stale artifact、version/no-daemon | `src/mcp/daemon-*.ts`、`__tests__/mcp-daemon.test.ts` | D3 |
| query worker pool、bounded queue、crash backstop | `src/mcp/query-pool.ts`、`src/mcp/query-worker.ts`、`__tests__/query-pool.test.ts` | D4 |
| explore ranking、adaptive projection、dynamic boundary | `src/mcp/tools.ts`、`__tests__/explore-*.test.ts`、`__tests__/dynamic-boundaries.test.ts` | A4、B5 |
| Rust route 与 Cargo workspace framework resolver | `src/resolution/frameworks/rust.ts`、`src/resolution/frameworks/cargo-workspace.ts` | A5、F1 |
| expected-symbol retrieval evaluation | `__tests__/evaluation/**`、`docs/SEARCH_QUALITY_LOOP.md` | E0、E3 |
| with/without 与 tool-surface ablation harness | `scripts/agent-eval/**`、`docs/benchmarks/**` | E1 |

以下实践进入 Track A、B、D、E、F：

| codegraph 实践 | Atlas 采用方式 |
|---|---|
| 单一综合 `codegraph_explore` | 增加确定性 `atlas explore`；A/B 后再决定 MCP 默认暴露面 |
| `impact` 与 changed-file `affected` | 增加反向图遍历，再连接 binding、scenario 和真实 test selector |
| source/target edge index 与 symbol search | 增加 derived query index，JSON shard 仍是正典 |
| 一次返回 source、path、blast radius | 返回受预算约束的源码与可解释图路径 |
| heuristic provenance 与 synthesis metadata | 保留 Atlas provenance，另加 confidence、dispatch、evidence、candidate |
| adaptive output sizing 与 sibling skeleton | 保留 path-spine body，压缩可互换的 off-spine implementation |
| relevance gate 先于 byte cap，预算是 ceiling | B5 不为“填满上下文”保留低相关内容；receipt 分开统计 retrieval 与 projection loss |
| path-scoped trace 远小于宽泛 explore，且小仓库也会 payload 膨胀 | 为 flow/trace 保留 compact spine，不预设所有问题都走 deep explore；E0/E1 记录 response bytes、read-back 与后续补查 |
| 查询期 dynamic boundary | 先作为不写图的 runtime-boundary hint；通过 mechanism corpus 后才晋升为 bounded candidate edge |
| pending file 只在成功 sync 后按 watermark 清除 | D3 查询必须局部标 stale；mid-sync event、锁冲突和失败不得丢 pending |
| changed-file resolution 加 orphan sweep | D2 使用 dependent frontier 和 recoverable work queue，防止中止后永久缺边 |
| bounded watcher、retry 与 explicit degrade | D3 对 OS watch、锁竞争和普通失败分别设上限并暴露 degraded |
| single daemon 与 stale lock/socket recovery | D3 加 worktree/version/start identity；保留 no-daemon 和静态 MCP discovery |
| lazy query workers、bounded queue 与 crash circuit breaker | D4 隔离 transport 和 CPU-heavy query；保留 typed overload 与零 worker 路径 |
| 真实 Agent with/without 和 tool-surface ablation | E1 使用 A/B/C 三臂，分别证明 Atlas 总价值与 B5 增量价值 |
| expected symbol、recall 与 MRR corpus | E3 吸收机器评分，但增加 forbidden path、evidence、stale 与答案 rubric，避免只测“出现过” |
| 多语言 extraction 加 framework resolver | A5/F1 分离 core provider、mechanism enricher 与 framework pack；不把 polyglot parser 搬进 Rust Atlas |

明确不复制的内容：

- 在 Rust Atlas 内实现 polyglot tree-sitter 架构。
- 把 heuristic dynamic edge 伪装成 compiler fact。
- 用“永远新鲜”隐藏 pending 或 layer-specific freshness。
- 预先绑定 SQLite、Node daemon 或 installer-side agent configuration。
- 未经 Atlas A/B 就默认只暴露一个 MCP 工具。
- 把宽泛 `explore` 当成所有问题的固定入口，或用工具调用次数下降掩盖单次 payload 膨胀。
- 因为 watcher 已启动就宣称 index 永远新鲜，或在 pending/degraded 时继续给出确定性结论。
- 把 query-time dynamic boundary hint 写成 exact call edge，或让 heuristic edge 参与归档证明。
- 把 handler/service 目录名、`*_handler` 命名或 Cargo dependency 单独当作 exact framework edge。
- 为了 transport 不超时而把 `busy` 包装成成功结果，或让 worker fallback 混用不同 generation。
- 仅靠 expected-symbol recall 宣布架构流正确；错误 path 与错误 provenance 同样是失败。
- 把 benchmark 的 canonical matrix、judge 或失败样本只保存在 `/tmp` 和个人环境中。
- 在 agent-spec 已有 Contract selector/test obligation 时仍只按文件名选测试。
- 直接采用 codegraph 的 benchmark 百分比作为 Atlas 验收阈值。其方法可以借鉴，但 Atlas
  必须建立自己的 Rust baseline。

## 6. 交付顺序

推荐顺序优先改善 Agent 可用性，不等待最重的 compiler integration：

| 顺序 | 交付物 | 依赖 | 当前优先原因 |
|---|---|---|---|
| 1 | E0 Rust benchmark baseline | 已交付图 | Wave 1 已交付离线 corpus、plan 与 receipt summary；尚无真实 A/B 结果 |
| 2 | A2 edge evidence 加 B1 query index | syn 与 SCIP | Wave 1 已交付 schema v6、atomic index 与 deterministic search |
| 3 | D1 worktree 与 layered freshness | 已交付 stale model | Wave 1 已交付 identity、layer status 与 provider/binding authority gate |
| 4 | B2/B3 explore 与 flow（已交付） | E0、A2、B1、D1 | 给 Agent 一个内容充分的架构查询并延续离线评测契约 |
| 5 | B4 impact 与 affected code（已交付） | E0、B1、B3 | 提供确定性反向遍历与同一 receipt 指标 |
| 6 | C1/C2/C3 intent-aware affected、bundle 与 replay（已交付） | B4、已交付 binding/quality planning | 连接代码变更、需求、测试、工具、skill 与同 run evidence |
| 7 | A3 MIR overlay consumer（已交付） | A2、B1 | 已提供 compiler evidence 接入与治理；官方 producer 单独交付 |
| 8 | A4 dynamic-dispatch enricher（trait v1 已交付） | A3、B3 | 已覆盖 trait method；其余机制按独立精度门扩展 |
| 9 | E3 query quality regression loop（基础闭环已交付） | E0、B2/B3/B4 | 两层 corpus、live fixture probe 与 fingerprinted score gate 已提供晋升门；fresh pinned capture 持续执行 |
| 10 | A4.1 runtime-boundary hints（已交付） | A4 trait v1、B3、E3 | fresh-source AST query hint 已解释静态图终点，未把候选写成边 |
| 11 | D2 incremental hardening（已交付） | B1、D1 | generation transaction、dependent frontier、orphan recovery 与 zero-change fast path 已交付 |
| 12 | D3 watch 与 daemon（已交付） | D2 | 已用 pending watermark、bounded retry、reader lease 和 typed degraded 增加可选实时性能，保留 no-daemon parity |
| 13 | B5 query context compiler（已交付） | B2/B3/B4、E3 | 已分离 retrieval 与 projection，交付 evidence priority、omission manifest 和双层 receipt |
| 14 | D4 concurrent query serving（已交付 opt-in prototype） | D3、B5 load profile | 已增加 bounded worker/queue、transport/control isolation、typed outcomes 与 20-run receipt；没有 E1 并发收益证据时保持 direct mode |
| 15 | E1 real Agent A/B（harness 已交付，真实结论 pending） | E3、B5/D4 候选面 | 已固化 A/B/C、失败保留、MAD gate 与独立 serving burst；等待真实 receipt 和人工接受后才决定默认入口、预算和并发策略 |
| 16 | F1 provider adapter kit（已交付） | Rust C1、D1/D2 语义已验证 | 已固化 provider/enricher schema、bounded process、atomic publish 与八项 conformance contract |
| 17 | A5 Rust framework semantic packs | A4、E3、F1、真实 framework gap | 每次只交付一个 corpus 驱动、可禁用的 framework pack |
| 18 | F2 non-Rust providers | F1、明确项目需求 | 按需求接 generic SCIP、tree-sitter 或本地 Code Graph adapter |

第一轮实施使用三个独立合约：

1. `REQ-ATLAS-AGENT-EVALUATION` → `task-atlas-agent-evaluation`
2. `REQ-ATLAS-EDGE-EVIDENCE-INDEX` → `task-atlas-edge-evidence-index`
3. `REQ-ATLAS-WORKTREE-FRESHNESS` → `task-atlas-worktree-layered-freshness`

三份 requirement 均为 `accepted`，并由 lifecycle、replay、trace 和治理门禁形成 Wave 1
证据。Wave 1 当时的完成范围严格限于 E0、A2/B1、D1；后续交付状态由下面各轮记录与
能力轨道条目覆盖。

第二轮实施使用一个聚合消费层合约：

1. `REQ-ATLAS-EXPLORE-FLOW-IMPACT` → `task-atlas-explore-flow-impact`

该 requirement 为 `accepted`。Wave 2 已交付 B2/B3/B4 library 与 CLI、opt-in frozen
`atlas_explore` MCP，以及 response bytes、read-back、后续查询和 truncation 的离线 receipt
指标。Contract lifecycle、requirement replay 与 trace graph 是本轮交付证据；真实 Agent A/B
仍未执行，因此默认 MCP surface 不变。

第三轮实施使用三个 Intent-Aware 合约：

1. `REQ-INTENT-AWARE-AFFECTED` → `task-intent-aware-affected`
2. `REQ-AFFECTED-EXECUTION-BUNDLE` → `task-affected-execution-bundle`
3. `REQ-AFFECTED-FAILURE-REPLAY` → `task-affected-failure-replay`

三份 requirement 均为 `accepted`。C1 将 provider-neutral code impact 与 requirement、leaf
work unit、Task Contract、scenario、显式 test selector 和 worktree/VCS 证据连接；C2 生成
可执行但不把候选测试冒充权威测试的 affected bundle；C3 以同一稳定 `run_id` 持久化并
重放 intent-impact、quality outcome 与 lifecycle evidence。风险 A lifecycle、独立复审和
requirement governance gate 是本轮交付证据。

第四轮实施使用一个 MIR overlay 合约：

1. `REQ-ATLAS-MIR-OVERLAY` → `task-atlas-mir-layer`

该 requirement 为 `accepted`。本轮交付非默认 `mir` feature、
`rust-atlas/mir-overlay-v1` consumer、固定 argv producer adapter、精确 MIR call edge、函数
CFG summary、共享 query-index provenance precedence、结构化独立 freshness、严格 wire
校验、staged shard generation 与进程内 rollback。Charon 未通过 stable 兼容性门；官方
`rustc_public`
producer 仍是单独交付项，不能用测试 driver 冒充已交付的 compiler extractor。

第五轮实施使用一个 dynamic-dispatch 合约：

1. `REQ-ATLAS-DYNAMIC-DISPATCH` → `task-atlas-dynamic-dispatch`

该 requirement 为 `accepted`。本轮只交付 trait-method v1：显式 opt-in whole-graph pass、
bounded implementation candidates、64 fan-out fail-closed、默认 rebuild 清理，以及现有
flow/impact candidate traversal 复用。closure、channel、callback registry 和 framework
mechanism 仍明确留在后续范围。

第六轮实施使用一个 query-quality 合约：

1. `REQ-ATLAS-QUERY-QUALITY-REGRESSION` → `task-atlas-query-quality-regression`

该 requirement 为 `accepted`。本轮在 E0 evaluator 内增加严格的两层 golden corpus、
一一对应的 typed observation、symbol recall、MRR、path precision/recall、forbidden-hit、
evidence、diagnostic 和 query-cost 评分，以及带 corpus fingerprint 的原子 receipt。
默认测试使用当前 fixture graph 的真实 search/flow 输出而不是只验证手写 JSON 自洽；
pinned repository fresh capture、真实 Agent A/B 和默认 MCP surface 变化仍留在 E1。

第七轮实施使用一个 runtime-boundary 合约：

1. `REQ-ATLAS-RUNTIME-BOUNDARY-HINTS` → `task-atlas-runtime-boundary-hints`

该 requirement 为 `accepted`。本轮只在 disconnected、非 truncated 且 endpoint 已解析的
`flow` 上按 source-first 顺序扫描 fresh source 和静态可达 function body，使用 `syn` AST 区分
async task、channel、callback registry、reflection 与 framework route。结果携带 source site、
mechanism、静态 key、候选文本、最多 16 个 canonical candidate、`authority: query-hint` 与
`confidence: heuristic`；查询最多扫描 8 个节点、200000 bytes 并输出 4 个 hint，超限显式
truncated。它不写 shard、不改变 fingerprint，也不进入 impact、affected、binding、lifecycle
或 archive authority。扫描只进入 name、signature、span 唯一匹配的 function AST，同行 sibling
不能贡献 site；存储/解析 signature 共享 whitespace canonicalization，receiver role 只接受实际
receiver chain 中完整或下划线分隔的 AST identifier，忽略 arguments、index values 与 literal；
`crate`、`self`、`super`、`Self` 与 qualified candidate path 在 source context 中规范化，
qualified-self 保留类型、trait、generic arguments 与 member 后再查 index；default trait method
通过 `Contains` parent 区分 trait container 与 declaration module，并把 `<Self as Trait>::member`
解析到 trait declaration member。framework route 同时保留 `route(path, handler)` 与
`service(handler)` 的 continuation；generic reflection text 保持原样，但按其 indexed type
declaration 解析。reflection lookup 只保留 type-namespace declaration，async/callback/route
lookup 只保留 function declaration，并在 fan-out 计数前完成过滤。`crate::Type::method`、
`self::Type::method` 与 source-relative associated callable 通过 type declaration 展开到
canonical inherent-impl method symbol。bare candidate 优先 source-module exact match，不把
sibling module 的同名 symbol 并入候选。frontier 与 AST scanner 共享 per-file source cache，并在新 source read 前应用
8-node/200000-byte budget。stale source 不扩展后继，
SCIP/MIR edge 只有对应 layer fresh 时才能扩展。E3 的
runtime-boundary live fixture 使用 fresh SCIP helper edge，并直接对
生产 flow 发出的 expected continuation、source evidence 与 exact diagnostic 做回归评分；A4.2
持久化候选边仍受每个 mechanism 的独立晋升门约束。

第八轮实施使用一个 incremental-hardening 合约：

1. `REQ-ATLAS-INCREMENTAL-HARDENING` → `task-atlas-incremental-hardening`

该 requirement 为 `accepted`。本轮交付 content-addressed Cargo input plan、每次重建的 source
module ownership、bounded reverse-dependent frontier、frontier overflow full fallback、可恢复
orphan queue、batch cancellation 与 working-byte ceiling。完整 meta、shards、query index、input
plan 和 overlay capability 在 immutable generation 中完成后才原子切换 `CURRENT.json`；所有
query/status surface 固定并报告同一个 generation id。健康 zero-change build 校验 artifact digest
后不创建 staging、不运行 resolution/validation，也不重写 authority/control 文件。10-case
fixture matrix 覆盖 cold、zero、edit、delete、manifest、overflow、overlay、cancel、commit failure
和 recovery。stale query refresh 保留 committed Cargo inputs，capability 切换进入显式 full
frontier；frontier/resolution/validation 流式处理 shard batch，byte gate 覆盖 source、serialized
graph 与 overlay。post-commit orphan cleanup 失败会 warning 并把 queue 重绑定到新 generation，
供下一次 build 恢复。

第九轮实施使用一个 live-runtime 合约：

1. `REQ-ATLAS-LIVE-RUNTIME` → `task-atlas-live-runtime`

该 requirement 为 `accepted`。本轮在 D2 immutable generation 上交付 bounded watcher、
pending watermark、writer/ordinary 独立 retry、typed degraded lifecycle、daemon identity
handshake 与 supervision、single writer、跨进程 reader lease、fail-closed generation/staging
reclamation，以及静态 MCP discovery 和 no-daemon query parity。验收矩阵位于
`fixtures/atlas/live-runtime/matrix.json`，完整操作与权威边界见
`docs/atlas-live-runtime.md`。

第十轮实施使用一个 Agent adoption 合约：

1. `REQ-ATLAS-AGENT-AB-GATE` → `task-atlas-agent-ab-gate`

该 requirement 为 `accepted`。本轮交付严格三臂 experiment/plan/receipt/gate、失败 run 保留、
correctness/freshness-first 判定、由 matched baseline median/MAD 派生的 benefit/tie，以及独立的
四 profile direct/worker burst gate。两个 runner 只接受显式外部 executable，默认测试不启动
Agent、模型或网络。checked-in Agent plan 只有 72 个待执行 run，serving manifest 默认禁用；
仓库没有真实 receipt、人工接受或性能通过结论，因此默认 MCP、B5 profile 与 D4 direct mode
保持不变。

第十一轮实施使用一个 external provider 合约：

1. `REQ-CODE-GRAPH-PROVIDER-KIT` → `task-code-graph-provider-kit`

该 requirement 为 `accepted`。本轮交付独立的 `agent-spec-code-graph-provider` Rust SDK、
strict manifest/project registration、互斥的 extraction/enrichment payload、host-derived
fingerprint、worktree/freshness/path/provenance 校验、literal argv bounded process、timeout/
cancellation、same-directory atomic publish，以及 stable-id、determinism、partial、stale/
worktree、schema、output limit、cancellation 和 atomic publish 八项 conformance receipt。
`atlas provider validate|conformance` 只消费显式本地配置；checked-in shell fixture 只证明
协议，不代表 F2 非 Rust 语言支持，也不进入默认 build、bind 或 requirements 流程。

## 7. 旧 Phase 映射

历史文档和合约使用过重叠的 Phase 编号。保留历史名称以维持 trace，并按下表理解：

| 历史标签 | 当前轨道状态 |
|---|---|
| 原始 Phase 1 Rust graph | A0 与 B0，已交付 |
| syn hardening Phase 1 | A0，已交付 |
| SCIP semantic Phase 2 | A1，已交付 |
| 原始 MIR Phase 2 | A3 consumer 已交付；官方 `rustc_public` producer 待单独发布 |
| 原始 KLL integration Phase 3 | C0，已交付 |
| polyglot Phase 3 | 改为 Rust Atlas 外部的 F1/F2 |
| 原 Phase 4 daemon/performance | D2/D3，位于 query/freshness 基础之后 |

Phase 0（`afca280`）与 Phase 1（`bb47849`）的逐缺陷审计证据表（共 10 项：
问题、修法、实测验收）见本文件 git 历史中对应时期的版本。

新文档与合约使用 track id 和描述性名称，不再增加数字 Phase。

## 8. Roadmap 交付完成定义

一个 roadmap item 只有满足全部适用条件才算已交付：

1. KLL requirement 已接受，Task Contract 有当前有效的 `satisfies` 链接。
2. parser、schema、migration、negative path、stale/worktree 行为有确定性测试。
3. active contract 的 `agent-spec lifecycle` 通过，且没有 skip/uncertain verdict。
4. 相关 graph invariant 与 provider capability check 通过。
5. 记录真实 workspace 数字，包括 unresolved/degraded case，而不只记录成功数量。
6. 改变默认查询面或输出形态的 Agent-facing 变化通过 Track E A/B gate；加性
   flag 与非默认命令不强制。
7. requirement trace 报告 `Honored`，replay 可以走到当前证据。
8. 文档与 skill guidance 反映最终命令面，且不把派生图事实提升为 KLL 真相。
9. Agent-facing retrieval 或 projection 变化通过 E3 固定 corpus；如果改变默认 MCP surface，
   还必须通过 E1 的真实 Agent A/B。
10. live runtime 变化证明事件不丢、失败不清 pending、单 writer、旧 generation 可读、
    degraded 可见和 no-daemon parity。
11. framework/enricher 变化有独立 manifest、真实正例、同名负例、inert control、版本边界、
    fan-out 上限与 false-positive/false-negative receipt。
12. 并发查询变化证明 transport liveness、bounded queue、cancellation、worker crash recovery、
    单 generation snapshot 和 typed overload；不能以吞吐提升交换错误或不完整结果。

## 9. 已知边界

- external crate definition 只有在对应 semantic index 被纳入 SCIP 输入时才可见。
- 代码无法编译时仍可保留 syn fact，但 SCIP/MIR 可能不可用。
- reflection、runtime registration、build-script generated behavior 和 external service 不一定
  能被静态解析。
- 精确 impact analysis 受图覆盖率限制，结果必须暴露 unresolved frontier。
- runtime-boundary hint 只说明“静态路径在此终止并可能继续”，不能证明某个候选一定执行。
- framework route/registration 的静态表示受宏展开、feature flag 和版本影响；未被 compiler 或
  typed API 证明的 target 只能是 candidate/hint。
- watcher 与 daemon 只能缩短 stale window，不能消除 analyzer latency、失败或外部生成代码
  带来的不确定性。
- worker pool 只能隔离查询 CPU 与 transport，不能提高图覆盖率，也不能修复 stale snapshot。
- 小仓库可能无法摊薄 index/MCP overhead；E0 必须保留这个 tie zone，不能隐藏它。
- Wiki 可以引用 Atlas fact，但仍是派生 working memory，不能替代 graph freshness、KLL
  requirement 或 lifecycle evidence。
