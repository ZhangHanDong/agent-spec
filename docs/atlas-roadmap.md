# Rust Atlas Roadmap：从可信 Rust 图到意图感知的代码智能

> 当前正典 roadmap，修订于 2026-07-20。状态基线：`agent-spec` 1.1.0、
> `rust-atlas` 0.2.0、代码基线 `1633696`（含 PR #6：graph load 的
> schema-mismatch 拒绝、spec_verify 构建失败判 Uncertain 而非 Fail），roadmap
> 基线 `7066d91`。首轮治理工件已接受，但实现状态仍以本文件各 track 为准。
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

## 2. 已交付基线

以下能力已经在生产代码和历史合约中实现。表中“已交付”描述代码基线；本次新增的
`REQ-ATLAS-SCIP-SEMANTIC` 用于修复历史上两个 Task Contract 共同满足
`REQ-INTENT-CODE-LINKER` 的归属冲突，其 liveness 要在下一次 lifecycle/replay 后单独
成为 `Honored`，不能因代码已经存在而直接推断。

| 能力 | 状态 | 证据 |
|---|---|---|
| syn 图与分片存储 | 已交付 | `REQ-RUST-ATLAS`、`specs/task-rust-atlas-code-graph.spec.md` |
| syn 正确性硬化 | 已交付 | workspace 布局、唯一 id、item 覆盖、诚实的 unresolved 边 |
| SCIP 语义 overlay | 已交付 | `REQ-ATLAS-SCIP-SEMANTIC`、`specs/task-atlas-scip-semantic.spec.md`、schema v4 |
| schema-version 门 | 已交付 | `read_meta` 强校验：不匹配即 `SchemaMismatch`，查询路径响亮失败并提示重建，build 降级全量重建（e90fcb5） |
| provider-neutral Code Graph IR 与 binding | 已交付 | `REQ-CODE-GRAPH-IR`、`specs/task-code-graph-ir-bindings.spec.md` |
| Contract 符号与 typed trace 集成 | 已交付 | `REQ-INTENT-CODE-LINKER`、`specs/task-atlas-kll-integration.spec.md` |
| Quality Planning 与 Execution Bundle | 已交付 | `REQ-QUALITY-PLANNING`、`specs/task-quality-planning-bundles.spec.md` |

当前图能力包括：

- Cargo-aware workspace 布局和按源文件 blake3 失效。
- stable-toolchain syn 提取和 parse-error 降级。
- 直接读取 rust-analyzer SCIP protobuf。
- 带 provenance 与 resolution 的 `calls`、`uses-type`、`references`、
  `impls-trait`、`impl-for` 边。
- tree、node、refs、impls、status 的 CLI 与 MCP 查询。
- stale-aware Contract symbol、code binding、lifecycle 检查和 typed trace target。
- graph load 的 schema-version 强校验：旧 schema shard 不静默半读，拒绝并给出
  可执行的 rebuild 提示。

已有语义规模足以支持更强的消费层。在审计过的 grok-build workspace 上，SCIP overlay
约产生 120,000 条 `calls`、84,000 条 `uses-type` 和 415,000 条 `references`。下一阶段
的主要瓶颈已经不只是事实提取，而是检索、遍历、解释和增量服务。

## 3. 当前缺口

| 缺口 | 后果 |
|---|---|
| Edge 没有 call-site span 和 analyzer evidence | 无法说明一条路径中的每一跳为何存在、发生在哪里 |
| 查询加载并扫描全部 JSON shard | 反向遍历和大 workspace flow 查询无法扩展 |
| 没有确定性符号搜索与候选排序 | Agent 必须事先知道 canonical symbol id |
| 没有 `explore`、`flow`、`impact`、`affected` 综合查询 | Agent 仍需拼接多个低层调用和源码读取 |
| 影响分析停留在代码事实 | 代码变更还不能直接追到需求、scenario、测试和质量门禁 |
| freshness 尚未形成完整的分层/worktree 契约 | 旧语义层或借错 worktree 的图可能被误读 |
| 没有真实 Rust Agent benchmark gate | 查询形态变化无法证明 Agent 实际受益 |
| 零变更 rebuild 仍有明显全图地板 | 直接上 daemon 只会隐藏低效 resolve/validate，而不是解决它 |

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

状态：Requirement 与 Task Contract 已接受，实现待开始。

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

计划合约：`task-atlas-edge-evidence-index`。

#### A3. MIR overlay

状态：已在 `specs/roadmap/task-atlas-mir-layer.spec.md` 规划。

- 优先评估 Charon；兼容性不足时再考虑 `rustc_public` driver。
- 增加精确 MIR call edge 和 per-function CFG summary。
- nightly 和 extractor version 要求必须 feature-gated。
- 默认保留 generic form，不展开所有 monomorphized instance。
- MIR 不可用时降级到 syn 加 SCIP，并返回 typed diagnostic。

MIR 应增强一个已经能够解释 evidence 和 flow 的消费层。因此它依赖 A2 和第一版查询
索引，但不阻塞这些高收益能力先落地。

#### A4. Rust dynamic-dispatch enricher

状态：未来工作，位于 A3 之后。

候选机制包括 trait object、closure/function pointer、async task spawn、channel、callback
registry 和选定的 Rust framework route。whole-graph 或 framework 推理必须与 core parser
隔离，并输出 bounded candidate 与显式 confidence。

### Track B：Agent Query and Retrieval

#### B0. 现有低层查询

状态：已交付。

library、CLI 和 MCP 已提供 tree、query、refs、impls、status。即使未来默认 Agent surface
收敛，这些稳定 primitive 仍然保留。

#### B1. Search、disambiguation 与 derived query index

状态：Requirement 与 Task Contract 已接受；与 A2 在同一合约交付，实现待开始。

- 为 symbol/name、file-to-node、incoming/outgoing edge by kind 建立可重建索引。
- 支持 exact、qualified、segmented identifier 与 deterministic fuzzy search。
- 返回排序后的 ambiguity candidate、canonical id 与 location。
- 选存储依赖前，对 JSON-side index、SQLite 或其他 embedded index 做 benchmark。
- JSON shard 仍是正典存储，index recreation 必须 atomic。

#### B2. 综合查询 `atlas explore`

状态：B1 之后。

`atlas explore` 是确定性组合查询，不在 Atlas 内调用 LLM。它从输入中提取 identifier 和
path，查询图后一次返回受预算约束的结果：

- 相关 symbol 与新鲜 source excerpt；
- relationship map 和关键 path spine；
- caller、callee、implementation 与 blast-radius summary；
- 每一跳的 site、provenance、resolution、dispatch、confidence；
- stale、unavailable、ambiguous 与 truncation diagnostic。

输出必须支持至少两种确定性预算：面向路径问题的 compact spine，以及面向架构解释的
bounded deep context。不能因为综合查询存在，就强迫所有问题承担同样的 source payload。

只有当前源码 hash 与选择它的图层匹配时，才能内联 source excerpt。frozen stale query
不能把旧图路径和未标注的当前源码混在一起。

现有低层 CLI 继续保留。MCP 是否默认只列出 `atlas_explore`，必须由 Track E 的 Atlas
A/B 结果决定，不能仅凭其他项目经验直接修改。

计划合约：`task-atlas-explore-flow-impact`。

#### B3. Flow query

状态：与 B2 一起规划。

```text
atlas flow --from <symbol> --to <symbol>
atlas flow --through <symbol>
```

- 返回有界的 shortest path 和 highest-confidence path。
- dispatch 有歧义时保留 alternative path。
- 区分 no-path、capability unavailable 和 search truncated。
- 为 spine node 提供足够源码，避免为了理解一条 flow 打开所有参与文件。

#### B4. Code impact 与 affected test

状态：与 B2 同期或紧随其后。

```text
atlas impact <symbol> --depth <n>
git diff --name-only | atlas affected --stdin
```

- 反向遍历 call、reference、type use、impl 与 containment edge。
- 输入支持 symbol、file、staged change、worktree change 和 commit range。
- 每个 affected node 返回 path 与 distance，不只返回平铺列表。
- 不得仅凭测试文件名模式断言确定性 test coverage。
- 输出 provider-neutral result，供 Intent-Code Linker 与 test obligation、Contract
  selector 连接。

### Track C：Intent-Aware Impact and Execution

#### C0. Binding 与 lifecycle 集成

状态：已交付。

- ready work unit 可以绑定 fresh provider node。
- Task Contract 可以声明 canonical symbol。
- lifecycle 检查 missing symbol 与 stale graph。
- trace target 记录 provider、node、file、provenance 和 graph fingerprint。

#### C1. Intent-aware `affected`

状态：B4 之后的下一步。

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
test selector，或者 required provider 不可用。不得静默丢弃这些路径。

计划合约：`task-intent-aware-affected`。

#### C2. Affected execution bundle

状态：C1 之后。

- 根据 graph impact 和 requirement risk 为一个 work unit 选择 fast check 与 acceptance
  gate。
- 通过显式 Test selector 和 test obligation 选择测试；文件名 heuristic 只能提议候选。
- 从 project guidance 解析 required skill，记录 immutable skill receipt，但不把 receipt
  当作通过证据。
- 解释每个 tool、test、skill 被纳入的原因。

#### C3. Failure explanation 与 replay 增强

状态：C1 之后。

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

### Track D：Live Runtime and Large Workspaces

#### D1. Worktree identity 与 layered freshness

状态：Requirement 与 Task Contract 已接受；A2/B1 之后实施。

- metadata 包含 git common dir、worktree root 与 graph root。
- 检测从其他 worktree 借用的图，并拒绝确定性消费或清晰标注。
- 分别报告 syn、SCIP、MIR freshness。
- fingerprint 包含 analyzer 与 toolchain version。
- binding、lifecycle 和 query gate 消费同一 freshness result。
- 已交付的 schema-version 拒绝（e90fcb5）是本轨道第一个落地件，合约须把它纳入
  回归场景。

计划合约：`task-atlas-worktree-layered-freshness`。

#### D2. 增量 resolution 与 validation

状态：daemon 之前。

- 相关输入未变时缓存 Cargo metadata。
- 只更新 changed declaration 及其 dependent 的 symbol/reverse-edge index。
- 零变更 rebuild 避免全图 resolution 与 validation。
- 增加 bounded memory、cancellation、atomic shard swap 和 lock recovery。
- 分别测量 cold build、zero-change rebuild、single-file edit 和 large overlay。

#### D3. 可选 watch 与 daemon mode

状态：D1、D2 之后。

- 使用有界 OS resource 和 debounce 监听源码及 analyzer output。
- 静态 MCP tool discovery 不依赖 index warm-up。
- crash 和 stale lock 恢复时，不返回部分提交的图。
- 为 CI、sandbox 和确定性运行保留显式 no-daemon mode。
- 不宣称“永远新鲜”，必须暴露 pending sync 与 degraded watch state。

Daemon 是正确增量模型之上的优化，不是对低效全图重算的遮蔽。

### Track E：Evaluation and Adoption

#### E0. Rust benchmark baseline

状态：Requirement 与 Task Contract 已接受，实现优先于默认 MCP surface 变化。

建立可复现 corpus，覆盖 small、medium、large Rust workspace。每次能力变更至少测试以下
问题类型：

- symbol 与 implementation discovery；
- request/event flow reconstruction；
- change impact 与 affected test；
- 需要编辑与验证的 implementation task；
- stale、SCIP unavailable、compile-failing、alternate-worktree 场景。

#### E1. Agent A/B gate

状态：与 B2 一起规划。

- 使用相同 model、prompt、repository revision、permission 和 cold/warm condition。
- 对比 Atlas enabled 与 built-in Read/Grep exploration。
- 每个 arm 至少运行三次，报告 median 与 variance。
- 先测 answer correctness，再测 file read、graph call、total tool call、wall-clock、
  context size 与 cost。
- 不允许 correctness regression，也不允许把 stale result 展示为 fresh。
- medium/large repo 应显著减少 Read/Grep 和总 tool call；具体阈值来自 E0，不复制其他
  项目的 benchmark 数字。

#### E2. Coverage 与 honesty metric

状态：持续执行。

按 workspace 与 provenance layer 报告：

- resolved、unresolved、external、ambiguous edge；
- 有 resolved cross-file dependent 的 file 与 symbol；
- exact path 与 bounded-candidate path；
- fixture 中的 false positive 与 false negative；
- `atlas explore` 后的 read-back；
- query truncation 与 fallback rate。

### Track F：Provider Ecosystem

#### F0. Provider-neutral consumer contract

状态：已通过 `REQ-CODE-GRAPH-IR` 交付。

#### F1. External provider adapter kit

状态：Rust 路径通过 C1 验证之后。

- 文档化第三方 provider 的 capability discovery、node/edge projection、freshness、graph
  fingerprint 和 error normalization。
- 增加不依赖特定工具的 provider conformance fixture。
- adapter 必须可选且由项目配置；agent-spec 不绑定单一供应商或 orchestration system。

#### F2. 非 Rust provider

状态：需求驱动。

候选包括 generic SCIP adapter、独立 tree-sitter provider，或已有本地 Code Graph 工具的
adapter。它们投影到同一 Code Graph IR，并通过 provider conformance test，但不成为
`rust-atlas` 内部模块。

## 5. 从 codegraph 吸收的经验

本轮基于本地 checkout 审查了
[codegraph](https://github.com/colbymchenry/codegraph) `v1.3.1`
（commit `e552dc2`）。以下实践进入 Track B、D、E：

| codegraph 实践 | Atlas 采用方式 |
|---|---|
| 单一综合 `codegraph_explore` | 增加确定性 `atlas explore`；A/B 后再决定 MCP 默认暴露面 |
| `impact` 与 changed-file `affected` | 增加反向图遍历，再连接 binding、scenario 和真实 test selector |
| source/target edge index 与 symbol search | 增加 derived query index，JSON shard 仍是正典 |
| 一次返回 source、path、blast radius | 返回受预算约束的源码与可解释图路径 |
| heuristic provenance 与 synthesis metadata | 保留 Atlas provenance，另加 confidence、dispatch、evidence、candidate |
| adaptive output sizing 与 sibling skeleton | 保留 path-spine body，压缩可互换的 off-spine implementation |
| path-scoped trace 远小于宽泛 explore，且小仓库也会 payload 膨胀 | 为 flow/trace 保留 compact spine，不预设所有问题都走 deep explore；E0/E1 记录 response bytes、read-back 与后续补查 |
| watch、daemon、lock recovery、worktree mismatch | 先吸收 failure case 与测试，增量正确后再实现运行时 |
| 真实 Agent with/without A/B harness | 把 Agent 行为和答案正确性纳入 release evidence |

明确不复制的内容：

- 在 Rust Atlas 内实现 polyglot tree-sitter 架构。
- 把 heuristic dynamic edge 伪装成 compiler fact。
- 用“永远新鲜”隐藏 pending 或 layer-specific freshness。
- 预先绑定 SQLite、Node daemon 或 installer-side agent configuration。
- 未经 Atlas A/B 就默认只暴露一个 MCP 工具。
- 把宽泛 `explore` 当成所有问题的固定入口，或用工具调用次数下降掩盖单次 payload 膨胀。
- 在 agent-spec 已有 Contract selector/test obligation 时仍只按文件名选测试。
- 直接采用 codegraph 的 benchmark 百分比作为 Atlas 验收阈值。其方法可以借鉴，但 Atlas
  必须建立自己的 Rust baseline。

## 6. 交付顺序

推荐顺序优先改善 Agent 可用性，不等待最重的 compiler integration：

| 顺序 | 交付物 | 依赖 | 当前优先原因 |
|---|---|---|---|
| 1 | E0 Rust benchmark baseline | 已交付图 | 改查询 UX 前先建立证据 |
| 2 | A2 edge evidence 加 B1 query index | syn 与 SCIP | 让路径可解释且可扩展 |
| 3 | D1 worktree 与 layered freshness | 已交付 stale model | 防止从错误 snapshot 给出权威答案 |
| 4 | B2/B3 explore 与 flow | A2、B1、D1 | 给 Agent 一个内容充分的架构查询 |
| 5 | B4 impact 与 affected code | B1、B3 | 提供确定性反向遍历 |
| 6 | C1/C2 intent-aware affected bundle | B4、已交付 binding/quality planning | 连接代码变更、需求、测试、工具和 skill |
| 7 | A3 MIR overlay | A2、B1 | 为已经可消费的 flow 增加精度 |
| 8 | D2 incremental hardening | B1、D1 | 移除全图 rebuild 地板 |
| 9 | D3 watch 与 daemon | D2 | 在不隐藏 stale 的前提下增加实时性能 |
| 10 | F1/F2 provider ecosystem | Rust C1 已验证 | 泛化经过验证的合约，而不是提前猜抽象 |

第一轮实施使用三个独立合约：

1. `REQ-ATLAS-AGENT-EVALUATION` → `task-atlas-agent-evaluation`
2. `REQ-ATLAS-EDGE-EVIDENCE-INDEX` → `task-atlas-edge-evidence-index`
3. `REQ-ATLAS-WORKTREE-FRESHNESS` → `task-atlas-worktree-layered-freshness`

三份 requirement 均为 `accepted`，Task Contract lint 均为 100%；
`lint-knowledge --gate`、`requirements graph --gate` 与 `requirements plan --gate` 已通过。
这只表示输入与 DAG 可执行，不表示生产代码已交付。三项 lifecycle 与 roadmap 完成定义
全部通过后，再启动 `task-atlas-explore-flow-impact`。

## 7. 旧 Phase 映射

历史文档和合约使用过重叠的 Phase 编号。保留历史名称以维持 trace，并按下表理解：

| 历史标签 | 当前轨道状态 |
|---|---|
| 原始 Phase 1 Rust graph | A0 与 B0，已交付 |
| syn hardening Phase 1 | A0，已交付 |
| SCIP semantic Phase 2 | A1，已交付 |
| 原始 MIR Phase 2 | A3，待实施 |
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

## 9. 已知边界

- external crate definition 只有在对应 semantic index 被纳入 SCIP 输入时才可见。
- 代码无法编译时仍可保留 syn fact，但 SCIP/MIR 可能不可用。
- reflection、runtime registration、build-script generated behavior 和 external service 不一定
  能被静态解析。
- 精确 impact analysis 受图覆盖率限制，结果必须暴露 unresolved frontier。
- 小仓库可能无法摊薄 index/MCP overhead；E0 必须保留这个 tie zone，不能隐藏它。
- Wiki 可以引用 Atlas fact，但仍是派生 working memory，不能替代 graph freshness、KLL
  requirement 或 lifecycle evidence。
