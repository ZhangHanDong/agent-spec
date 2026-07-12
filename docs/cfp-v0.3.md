# CFP-0001: ClaimFlow Protocol (CFP) v0.3

| 字段 | 值 |
|------|------|
| 协议名称 | ClaimFlow Protocol |
| 缩写 | CFP |
| 文档编号 | CFP-0001 |
| 版本 | 0.3 (Draft) |
| 状态 | Draft for Discussion |
| 日期 | 2026-05-11 |
| 编辑 | AlexZhang (张汉东) |
| 协作 | Claude (Anthropic) |
| 反馈 | 通过 GitHub Issues 提交 |
| 前序版本 | v0.1, v0.2 (2026-05-11) |

---

## 摘要

ClaimFlow Protocol (CFP) 是一套面向 AI 与人类协作时代的内容协议栈。它不以排版为第一性原理,而以**类型化主张** (typed claim) 为第一性原理;它不把文档视为静态文本,而视为可验证、可追踪、可演化、可执行的状态流。

HTML 解决了"内容如何在浏览器中显示",Markdown 解决了"内容如何被人类轻量书写"。两者都假设内容的本质是**给人看的视觉表示**。但在 AI Agent 大规模生产和消费内容(包括代码、应用、文档、多模态资产)的时代,真正需要被表达的是:谁在什么上下文中提出了什么主张、这些主张来自哪里、当前处于什么状态、是否可以被验证、执行、撤回、更新、追责。

CFP 通过**七层协议栈**解决这个问题。核心层(L0–L4)以 AI 优先方式设计,作为真理之源;界面层(L5–L6)以人类回路方式设计,提供审查、交互、渲染。多模态资产作为 L2 一等公民。生成式应用(Agent2App)作为协议核心应用场景,通过 L6 Runtime Family 模型支持 Web HTML、Native Makepad、文档导出等多种目标。

本文档定义 CFP v0.3 的核心数据结构、状态机、URI 寻址、序列化方式、Runtime 绑定、应用场景和实施路径。该版本为讨论草案,所有设计决策接受社区挑战。

**v0.3 相比 v0.2 的主要变化**(详见第 19 节变更历史):

- **明确 AI 优先 vs 人类回路的内部分层**(新增 4.4 节)
- **L5 增加完整的人机审查协议原语**(Propose / Review / Defer / Withdraw / ReviewChain)
- **L6 重构为 Rendering Runtime Families**(Web / Native AI / Document / Machine 四大族,HTML 与 Makepad 平起平坐)
- **Agent2App 重新定位为"Native AI 应用生成"为重点场景**
- **新增 Session 与 Memory 关系章节**(第 17 节)
- **新增设计原则 P13–P15**

---

## 1. 引言

### 1.1 背景与动机

当前内容生态的主流格式有两类:

- **HTML**(1991):为浏览器渲染设计
- **Markdown**(2004):为人类轻量书写设计

两者的共同假设是:**内容的本质是给人视觉消费的有损语义编码**。语义(这句话是事实还是观点、来源是什么、是否还有效)必须由人脑从视觉中重建。

2026 年的两个观察使这个假设的代价开始显现:

**观察 A**:AI 输出形态正在从 markdown 切换到 HTML / 生成式 UI。视觉带宽是大脑约 1/3 计算资源,markdown 已经触顶。从 raw text → markdown → HTML → 动态生成 UI → 神经渲染,演化路径清晰可见。

**观察 B**:Agent 生成的应用(无论 Web HTML Artifact 还是 Native UI)目前都是"一次性产物"——设计意图、决策依据、承诺清单都丢失在对话历史里。三周后修改时只能整体重新生成。

这两个观察揭示了同一个深层问题:**渲染层在快速演化,但缺乏稳定的协议层**。每个新渲染形态都从零开始,没有共享的"内容真理"。

CFP 的核心论断:

> **AI 时代需要的不是更好的渲染格式,而是渲染格式之下的协议层**。渲染层(L6)应当能自由演化——HTML、React、Makepad、Neural UI、未来未知形态——但内容本体(L0–L4)应当稳定、可追溯、可演化、可问责。

### 1.2 设计目标

CFP 旨在:

- **G1** 把内容的元信息(类型、状态、来源、承诺)从"人脑重建"提升为"协议层一等公民"
- **G2** 提供 Agent 与 Agent、Agent 与人之间的统一内容协议
- **G3** 在不破坏现有 Markdown/HTML 生态的前提下,提供渐进采纳路径
- **G4** 区分**协议本体**与**领域规范**,使协议保持精简
- **G5** 通过分层架构,使各层可以独立演化、独立标准化、独立采纳
- **G6** 把多模态资产视为可切片、可引用、可验证的语义证据对象,而非附件
- **G7** 为 Agent 生成的内容(代码、视图、应用)提供"出生证明 + 设计文档 + 承诺清单 + 演化记录"的协议级表达
- **G8**(v0.3 新增) 支持渲染运行时家族(Rendering Runtime Family)概念,使同一份内容对象图可绑定到 Web、Native、文档、机器等多种目标运行时
- **G9**(v0.3 新增) 提供人机协作审查的标准化原语,使 Agent 自主性与人类监督能在协议层精确衔接

### 1.3 非设计目标

CFP **不**试图:

- **N1** 替代 HTML/Markdown 作为渲染形态(它们在 L6 中作为渲染目标继续存在)
- **N2** 定义具体行业的承诺类型和合规要求(交给 agent-spec 等领域规范)
- **N3** 提供完整的身份认证与分布式信任系统(引用 DID、Verifiable Credentials 等现有标准)
- **N4** 强制采纳全部七层(每一层独立可用)
- **N5** 保证生成内容的法律效力(协议提供机器可验证机制,法律有效性由领域规范和司法管辖区决定)
- **N6**(v0.3 新增) 取代 Agent 系统的 Session 框架或 Memory 实现(CFP 是内容格式,不是运行时框架)
- **N7**(v0.3 新增) 绑定到特定 UI 框架(L6 设计为 Runtime 中立,Makepad 是参考实现之一,不是协议要求)

### 1.4 文档约定

本文档中的规范性术语含义遵循 RFC 2119:

- **MUST** / **必须**:绝对要求
- **SHOULD** / **应当**:强烈建议,偏离需说明理由
- **MAY** / **可以**:可选行为

伪代码使用 Rust 风格类型语法表达数据结构。这不约束实现语言。

---

## 2. 术语定义

| 术语 | 定义 |
|------|------|
| **Claim** | 一个类型化主张,CFP 的最小内容单元 |
| **Source** | 主张的来源对象 |
| **Media Asset** | 原始媒体对象 |
| **Media Segment** | 媒体资产的精确时空切片 |
| **Commitment** | 一个可机器验证的承诺 |
| **Document** | 一组 CFP 对象的命名集合 |
| **App Bundle** | Agent 生成的应用包,包含 CFP 对象图 + 代码工件 + Runtime 绑定 |
| **View** | 从 CFP 对象图派生的某种展示形式 |
| **Runtime Family**(v0.3) | L6 渲染运行时家族(Web / Native AI / Document / Machine) |
| **Runtime Binding**(v0.3) | App Bundle 绑定到的具体目标运行时声明 |
| **State Flow** | Claim 的生命周期状态机 |
| **Issuer** | 主张的发起方身份 |
| **CFP-URI** | CFP 对象的全局可寻址标识符 |
| **Capability** | 执行某个承诺所需的权限凭证 |
| **Trust Path** | 从某个 Claim 到其支撑来源的可验证溯源路径 |
| **Execution Proof** | 承诺履约的可回放证据 |
| **Review Chain**(v0.3) | 审查过程的持久化对象,记录审查的完整链路 |
| **Core Layer**(v0.3) | L0–L4,AI 优先的真理之源层 |
| **Interface Layer**(v0.3) | L5–L6,为人类回路保留的界面层 |

---

## 3. 设计原则

CFP 的所有设计决策必须满足以下原则。违反这些原则的设计提案应当被退回。

### P1 主张优先于展示 (Claim-First)

内容的核心是主张,不是排版。展示视图(HTML/Markdown/Makepad/PDF)是从主张图派生的视图之一,而非内容的本体。

### P2 元信息即一等公民 (First-Class Metadata)

类型、状态、来源、置信度、承诺、权限——这些元信息必须在协议层直接表达,不能埋在文本里靠 NLP 提取。

### P3 可降级到 Markdown (Markdown-Degradable)

任何 CFP 文档必须能渲染出一个对人类友好的 Markdown 视图。否则在人类回路退出前,新协议会饿死。

### P4 可升级自 Markdown (Markdown-Upgradable)

CFP 必须支持从现有 Markdown/HTML 中渐进提取出结构化对象。

### P5 每一层独立可用 (Layer Independence)

L0 没有 L4 也能用,L2 没有 L3 也能跑。社区必须能选择"先实现某一层"。

### P6 协议瘦、规范厚 (Thin Protocol, Thick Specs)

CFP 只定义抽象原语。具体行业承诺模板、合规要求、审计规则交给上层领域规范。

### P7 机器优先,人类降级 (Machine-First, Human-Degradable)

CFP 的原生形态是结构化对象。Markdown/HTML/Makepad 视图是给人类的降级输出。

### P8 失败优雅降级 (Graceful Degradation)

CFP 解析失败、字段缺失、版本不匹配时,系统必须降级到"当作纯文本处理",而不是崩溃。

### P9 不发明可复用的轮子 (Reuse Over Reinvent)

CFP 必须复用现有成熟标准:JSON/CBOR、URI、HTTP、CRDT、DID、JSON Schema、Media Fragments URI、C2PA 等。

### P10 推迟一切可推迟的决策 (Defer When Possible)

不强行决定所有问题。明确标注 Open Issues。设计接受迭代。

### P11 多模态作为证据,不作为附件

图像、音频、视频不是文档的"插件",而是与文本主张同等级别的证据载体。

### P12 可演化优于一次性

CFP 优先服务于"长期演化的对象图",而非"一次性生成的快照"。

### P13 核心层 AI 优先,界面层人类回路 (v0.3 新增)

L0–L4 为 AI 消费和生产优化(机器精度、结构化、可订阅)。L5–L6 为人类参与优化(可读、可审查、可干预)。这一分裂是有意的架构选择,不是缺陷。

### P14 Runtime 中立性 (v0.3 新增)

CFP 的 L0–L4 真理之源层 **MUST** 在不同 runtime 间保持语义等价。同一份 CFP 对象图绑定到 Web Runtime 与 Native Runtime 应产生功能等价但渲染形态不同的 App Bundle。

### P15 人类回路是协议级承诺 (v0.3 新增)

L5 审查机制 **MUST** 不能被绕过。任何 Capability 为 `RequiresConfirmation` 的承诺,**MUST** 在协议层等待 Review 事件,**MUST NOT** 由实现自行决定跳过。这是协议对"Agent 自主性边界"的硬性保证。

---

## 4. 协议架构

### 4.1 分层模型

CFP 采用七层架构。

```
┌──────────────────────────────────────────────────┐
│ L6  Presentation View      展示视图层               │ ─┐
│     Runtime Family: Web / Native / Doc / Machine  │  │
├──────────────────────────────────────────────────┤  │  Interface Layer
│ L5  Interaction & Review   交互与审查层             │  │  (人类回路)
│     Ask/Expand/Challenge + Propose/Review/Defer   │  │
├══════════════════════════════════════════════════┤  │
│ L4  Commitment             承诺层                  │  │
│     可执行义务、能力、权限、履约验证                    │  │
├──────────────────────────────────────────────────┤  │  Transport
│ L3  Provenance             来源链层                │  │  (横切关注点)
│     引用、hash、信任路径、quote range               │  │
├──────────────────────────────────────────────────┤  │  JSON / CBOR
│ L2  Multimodal Evidence    多模态证据层              │  │  over HTTP/3,
│     Asset / Segment / Annotation / EditChain      │  │  stdio,
├──────────────────────────────────────────────────┤  │  WebSocket
│ L1  State Flow             状态流层                │  │  Core Layer
│     生命周期状态机、事件溯源、订阅                      │  │  (AI 优先)
├──────────────────────────────────────────────────┤  │
│ L0  Typed Claim            类型化主张层              │  │
│     8 种核心 ClaimKind                            │  │
└──────────────────────────────────────────────────┘ ─┘
```

**关键架构决策**:
- 展示视图层在最顶层(派生),不是最底层(基础)
- 横线分隔 Core Layer 与 Interface Layer(见 4.4 节)
- 传输层是横切关注点,不是协议栈某一层

### 4.2 层间依赖关系

| 层 | 依赖 | 被依赖于 |
|------|------|------|
| L0 Typed Claim | — | 所有上层 |
| L1 State Flow | L0 | L2, L3, L5 |
| L2 Multimodal Evidence | L0, L1 | L3, L4 |
| L3 Provenance | L0, L2 | L4, L5 |
| L4 Commitment | L0, L1, L2, L3 | L5 |
| L5 Interaction & Review | L0, L1, L2, L3, L4 | L6 |
| L6 Presentation View | L0, (可选 L1–L5) | — |

### 4.3 最小可用配置

- **MVP-Light**:L0 + L6(类型化主张 + Markdown 渲染)
- **MVP-Standard**:L0 + L1 + L3 + L6(加状态流和文本来源链)
- **MVP-Multimodal**:L0 + L1 + L2 + L3 + L6
- **MVP-Agent2App**:L0 + L1 + L4 + L6(支持 App Bundle)
- **MVP-Full**:所有七层

### 4.4 Core Layer 与 Interface Layer 的分裂 (v0.3 新增核心说明)

CFP 七层在"AI 优先"维度上不是单一的。

**Core Layer (L0–L4):AI 优先**

L0–L4 构成 CFP 的"真理之源"。这五层的设计目标是:

- 默认形态是结构化对象,不是人类渲染
- 默认消费者是 AI / Agent / 自动化系统
- 默认产出方式是机器生成
- AI 的认知状态(置信度、推理、识别)是协议一等公民

这五层删掉所有人类渲染,AI 依然能完整工作。反过来不成立——人类要消费 L0–L4 需要 L6 视图。

**Interface Layer (L5–L6):人类回路**

L5–L6 是 CFP 中专门为"保留人类参与"而存在的层。

- **L5 Interaction & Review**:人机协作的协议接口。审查、提问、反驳、订阅都假设有人类(或代表人类的高权限 Agent)在回路中
- **L6 Presentation View**:把核心层的对象图渲染为人类可消费的形式

**设计原则**:

- 核心层 **MUST** 在没有界面层的情况下独立完整运行(Agent 间通信场景)
- 界面层 **MUST** 能从核心层完全派生(不能成为权威源)
- 实现者 **SHOULD** 对核心层和界面层采取不同的优化策略:
  - 核心层优化机器消费效率(CBOR、批量订阅、并发写入、索引查询)
  - 界面层优化人机协作体验(响应时间、清晰 UI、防止决策疲劳)

**为什么这样分裂**:

完全 AI 优先的协议会让人类被排除在重要决策之外。完全人类优先的协议会浪费 AI 能力、退回到 Markdown 时代。CFP 的分裂结构同时实现两个目标:

- **核心层**让 AI 之间、AI 与系统之间能高带宽、结构化、可验证地协作
- **界面层**让人类能在关键决策点保留干预、审查、撤回的权力

这个分裂不是缺陷,是协议的核心架构选择。

---

## 5. L0: Typed Claim Layer 类型化主张层

### 5.1 Claim 对象

Claim 是 CFP 的最小内容单元。

```rust
struct Claim {
    // 标识
    id: CfpUri,
    kind: ClaimKind,

    // 内容
    text: String,
    structured: Option<Value>,

    // 元信息
    issuer: Identity,
    issued_at: Timestamp,
    language: LanguageTag,
    confidence: Option<f32>,

    // 关系
    links: Vec<Link>,

    // L1–L4 元信息
    state: Option<ClaimState>,
    source_refs: Vec<CfpUri>,
    evidence_refs: Vec<EvidenceRef>,

    // 扩展
    extensions: Map<NamespacedKey, Value>,
}

struct Link {
    predicate: LinkPredicate,
    target: CfpUri,
    strength: Option<f32>,
}

enum LinkPredicate {
    Supports, Refutes, DependsOn, DerivedFrom,
    Supersedes, Refines,
    Annotates, Quotes,
    Custom(NamespacedName),
}
```

### 5.2 Claim 类型清单

CFP v0.3 定义 8 种核心 ClaimKind。**MUST** 支持全部 8 种。

| Kind | 用途 | 关键字段 |
|------|------|------|
| **Fact** | 可验证的客观事实 | `verifiability`, `source_refs` |
| **Opinion** | 主观观点,立场明确 | `stance`, `basis` |
| **Inference** | 推理结论,必须暴露前提 | `premises`, `method` |
| **Hypothesis** | 假设,带验证条件 | `verification_conditions` |
| **Observation** | 第一人称观察 | `observed_by`, `evidence_refs` |
| **Intent** | 意图、目标、愿望 | `goal`, `actor` |
| **Promise** | 承诺(L4 完整形态) | 详见 L4 |
| **Question** | 待回答的问题 | `expects`, `status` |

**关键规范**:Opinion 类型 **MUST NOT** 被渲染为与 Fact 视觉上不可区分的形式。Inference 类型 **MUST** 有非空 `premises`。视觉模型(图像识别)输出 **MUST** 表达为 Observation。

### 5.3 类型扩展机制

垂直领域可使用反向域名命名空间:

```rust
kind: ClaimKind::Custom("io.agent-spec.AuditFinding")
kind: ClaimKind::Custom("org.medical.Diagnosis")
```

扩展类型 **MUST** 提供 schema 引用和降级策略。

---

## 6. L1: State Flow Layer 状态流层

### 6.1 Claim 生命周期

```rust
enum ClaimState {
    // 普通 Claim
    Draft, Claimed, Supported, Verified,
    Challenged, Refuted, Deprecated, Superseded,

    // Promise Claim
    Pending, Running, Fulfilled, Failed,
    Cancelled, Expired,
}
```

### 6.2 状态转换规则

状态转换 **MUST** 通过显式 Event 触发。

```rust
struct StateTransition {
    claim: CfpUri,
    from: ClaimState,
    to: ClaimState,
    triggered_by: Event,
    timestamp: Timestamp,
    actor: Identity,
    rationale: Option<String>,
}
```

### 6.3 事件溯源

CFP 采用事件溯源作为状态管理底层模型:

- 每个 Claim 的当前状态 = 创建事件 + 所有变更事件的回放结果
- 状态本身可以推导,**MUST** 不作为权威源存储
- 事件流是不可变的、追加式的、可签名的

这一选择使 CFP 天然支持时间机器、撤回、多方协作、审计。

### 6.4 订阅模型

订阅一个 Claim **SHOULD** 返回:
1. 当前状态(事件回放结果)
2. 完整事件历史(可选)
3. 增量事件流(后续变更实时推送)

---

## 7. L2: Multimodal Evidence Layer 多模态证据层

### 7.1 设计哲学

多模态内容在 CFP 中是**一等证据公民**,不是附件。通过四种核心对象表达:

- **Media Asset**:原始媒体对象
- **Media Segment**:媒体的精确时空切片
- **Annotation**:对 Segment 的标注(作为 Observation Claim 实例)
- **Edit Chain**:媒体的生成、剪辑、转写链路

### 7.2 Media Asset

```rust
struct MediaAsset {
    id: CfpUri,
    kind: MediaKind,
    uri: Uri,
    mime_type: String,
    hash: Hash,                       // MUST 字段
    size_bytes: u64,

    created_at: Option<Timestamp>,
    duration_ms: Option<u64>,
    dimensions: Option<Dimensions>,
    sample_rate: Option<u32>,

    capture_context: Option<CaptureContext>,  // 隐私敏感
    authenticity: Option<AuthenticityInfo>,
}

enum MediaKind {
    Image, Audio, Video, Pdf,
    ScreenRecording, SensorData, ThreeDModel,
    Custom(NamespacedName),
}
```

**关键规范**:
- `hash` 字段 **MUST** 存在
- `capture_context` **MUST** 在传输前进行隐私脱敏检查
- LLM 生成媒体 **MUST** 通过 `authenticity.ai_generated: true` 显式标注

### 7.3 Media Segment

```rust
struct MediaSegment {
    id: CfpUri,
    asset: CfpUri,

    time_range: Option<TimeRange>,
    spatial_region: Option<SpatialRegion>,
    audio_channel: Option<u8>,
    speaker_label: Option<String>,  // 仅本地标签

    labels: Vec<Label>,
    derived: Option<DerivedData>,
}
```

**Media Fragments URI 集成**:CFP Segment **SHOULD** 同时支持 W3C 标准:

```
cfp://example.com/media-001#t=12.3,18.7
cfp://example.com/media-001#xywh=420,680,310,160
```

### 7.4 派生层规范

**关键设计原则**:转写文本、视觉识别结果、音频分析输出 **MUST** 被视为**派生层**:

- 转写 **MUST** 表达为带 `generated_by` + `confidence` + `state` 的独立 Claim
- 视觉识别 **MUST** 表达为 Observation Claim
- 派生数据 **MUST NOT** 替代原始媒体作为权威源

这一设计的关键好处:派生数据出错时,**只影响该 Claim,不污染原始媒体**。

### 7.5 视频作为状态流容器

视频时间轴上的状态变化 **SHOULD** 表达为绑定到 Segment 的 L1 Event。视频状态流自动复用 L1 全部能力。

### 7.6 Edit Chain 与真实性

媒体的剪辑历史、生成模型标记、签名信息 **MUST** 通过 `AuthenticityInfo` 表达。

```rust
struct AuthenticityInfo {
    content_hash: Hash,
    signed_by_device: bool,
    device_attestation: Option<DeviceAttestation>,
    c2pa_manifest: Option<C2paManifestRef>,
    ai_generated: bool,
    generator: Option<Identity>,
    edit_history: Vec<EditOperation>,
    tamper_check: Option<TamperStatus>,
}
```

CFP 不试图"保证媒体绝对真实",而是**强制要求暴露完整生成、编辑、转写、摘要的链路**。

### 7.7 说话人识别的特殊处理

- `MediaSegment.speaker_label` **MUST** 仅作为本地标签
- 真实身份绑定 **MUST** 通过独立的 SpeakerIdentification Claim
- 该 Claim **MUST** 明确标注授权来源
- 实现 **MUST NOT** 在未经显式授权的情况下传输身份绑定

---

## 8. L3: Provenance Layer 来源链层

### 8.1 Source 对象

```rust
struct Source {
    id: CfpUri,
    kind: SourceKind,

    uri: Option<Uri>,
    title: Option<String>,
    media_asset: Option<CfpUri>,      // 多模态来源

    retrieved_at: Option<Timestamp>,
    content_hash: Option<Hash>,

    trust_level: TrustLevel,
    signature: Option<Signature>,

    supports: Vec<SupportRelation>,
}

enum SourceKind {
    WebPage, AcademicPaper, Book, Dataset,
    Code, Observation, Conversation,
    OfficialDocument, LlmOutput,
    MultimodalAsset,
    Custom(NamespacedName),
}
```

### 8.2 内容指纹与失效检测

Source 携带 `content_hash`。实现 **SHOULD** 提供 freshness check 工具:定期重新获取来源 URI,比对 hash。不一致时自动将关联 Claim 状态标记为 `Challenged` 或 `Deprecated`。

这一机制让 CFP 文档**主动承认自己可能过期**。

### 8.3 信任模型

Trust 是局部的、可配置的。CFP **MUST NOT** 假设存在全局可信源。

```rust
enum TrustLevel {
    Authoritative, Reliable, UserGenerated,
    LlmGenerated, Unknown,
}
```

**关键规范**:LLM 生成内容 **MUST** 默认标注为 `TrustLevel::LlmGenerated`,**MUST NOT** 默认升级为 `Authoritative`。

---

## 9. L4: Commitment Layer 承诺层

### 9.1 Commitment 对象

```rust
struct Commitment {
    id: CfpUri,

    committer: Identity,
    beneficiary: Identity,

    obligation: Obligation,
    conditions: Vec<Condition>,
    deadline: Option<Timestamp>,

    verification: VerificationMethod,
    capability_required: Option<Capability>,
    permission_level: PermissionLevel,

    on_breach: Option<BreachClause>,

    // v0.3 新增:审查策略
    review_policy: Option<ReviewPolicy>,

    state: CommitmentState,
    signed_at: Timestamp,
    signature: Signature,
}

enum Obligation {
    Deliver(CfpUri),
    Refrain(ActionPattern),
    Maintain(StateInvariant),
    Notify(EventPattern, Channel),
    Custom(NamespacedName, Value),
}

enum VerificationMethod {
    Automated(Tool),
    Witnessed(Identity),
    SelfReported,
}
```

### 9.2 Capability 与权限模型

```rust
struct Capability {
    action: NamespacedAction,
    scope: CapabilityScope,
    constraints: Vec<Constraint>,
}

enum PermissionLevel {
    ReadOnly,
    RequiresConfirmation,
    AutoExecutable,
    Forbidden,
}
```

**关键规范**:
- 默认权限级别 **MUST** 为 `RequiresConfirmation`
- 提升为 `AutoExecutable` **MUST** 显式声明,**SHOULD** 有作用域和速率限制
- 实现 **MUST** 拒绝执行权限级别为 `Forbidden` 的承诺,即使签名有效

### 9.3 Execution Proof

承诺履约可绑定多模态证据:

```rust
struct ExecutionProof {
    commitment: CfpUri,
    submitted_at: Timestamp,
    evidence: Vec<EvidenceRef>,
    claims: Vec<CfpUri>,
    logs: Option<CfpUri>,
    verified_by: Option<Identity>,
    verification_result: VerificationResult,
}
```

典型场景:Agent 执行 UI 测试承诺,履约证据包含屏幕录制 + 关键截图 + 操作日志 + 测试结果 Claims。

### 9.4 协议与领域规范的边界

CFP L4 提供承诺的**语法和数据结构**。它故意不定义:
- 哪些 Obligation 类型对哪些行业合规
- 违约扣多少信誉点是合理的
- 跨组织承诺的法律有效性

这些是**领域规范层**(如 agent-spec)的责任。

---

## 10. L5: Interaction & Review Layer 交互与审查层 (v0.3 大幅扩展)

L5 是 CFP 中专门为"人机协作"设计的层。它包含两类原语:

- **内容交互原语**(v0.1 已有):Ask / Expand / Challenge / Revise / Subscribe
- **审查协议原语**(v0.3 新增):Propose / Review / Defer / Withdraw

### 10.1 内容交互原语

```rust
enum InteractionEvent {
    // 内容层交互
    Ask {
        question: Claim,
        target: Option<CfpUri>,
    },
    Expand {
        target: CfpUri,
        depth: ExpandDepth,
    },
    Challenge {
        target: CfpUri,
        reason: Claim,
    },
    Revise {
        target: CfpUri,
        replacement: CfpUri,
    },
    Subscribe {
        target: CfpUri,
        events: Vec<EventFilter>,
    },

    // v0.3 新增:审查层交互
    Propose { ... },
    Review { ... },
    Defer { ... },
    Withdraw { ... },
}
```

### 10.2 审查协议原语 (v0.3 新增)

CFP v0.3 把"人机审查"提升为协议级机制,而非实现细节。

```rust
struct ProposeEvent {
    proposer: Identity,
    proposed: CfpUri,                // 被审查的对象(Commitment/Decision/Plan)
    rationale: CfpUri,               // 推理依据(Inference Claim)
    context: ReviewContext,
    urgency: Urgency,
}

struct ReviewContext {
    triggered_by: TriggerKind,
    related_commitments: Vec<CfpUri>,
    risk_level: RiskLevel,
    affected_capabilities: Vec<Capability>,
}

enum TriggerKind {
    BeforeAction(CfpUri),            // 类似 PreToolUse
    AfterAction(CfpUri),             // 类似 PostToolUse
    StateTransition(CfpUri),         // 状态机即将变化
    CommitmentCreation,
    CommitmentBreach(CfpUri),
    HighStakesInference(CfpUri),
    Custom(NamespacedName),
}

struct ReviewEvent {
    reviewer: Identity,
    target: CfpUri,
    verdict: ReviewVerdict,
    rationale: Option<String>,
    conditions: Vec<Condition>,
}

enum ReviewVerdict {
    Approve,                                  // 直接批准
    ApproveWithModification(CfpUri),          // 修改后批准
    ApproveWithCondition(Vec<Condition>),     // 附加条件批准
    Reject(String),                           // 拒绝并说明
    Escalate(Identity),                       // 升级
    RequestMoreInfo(Vec<Question>),           // 要求补充信息
    Defer(DeferCondition),                    // 暂缓
    Silent,                                   // 不表态
}

enum DeferCondition {
    Until(Timestamp),
    UntilCondition(CfpUri),
    UntilReviewer(Identity),
    Indefinite,
}

enum Urgency {
    Blocking,
    Async,
    FireAndForget,
    BestEffort(Duration),
}
```

### 10.3 ReviewChain 作为一等对象

审查过程本身是可追溯的对象:

```rust
struct ReviewChain {
    id: CfpUri,
    subject: CfpUri,                   // 被审查的对象
    events: Vec<ReviewEvent>,          // 完整审查历史
    current_state: ReviewState,
    final_verdict: Option<ReviewVerdict>,
    finalized_at: Option<Timestamp>,
}

enum ReviewState {
    Pending,
    UnderReview(Identity),
    AwaitingMoreInfo,
    Deferred(DeferCondition),
    Resolved(ReviewVerdict),
    Expired,
    Cascaded,
}
```

**关键设计**:**审查链路是持久的、可订阅的、可查询的协议对象**。三个月后任何争议都能追溯到"这个决策是谁在什么时间基于什么理由批准的"。

### 10.4 Commitment 的审查策略

L4 Commitment 的 `review_policy` 字段(见 9.1)定义审查行为:

```rust
struct ReviewPolicy {
    triggers: Vec<TriggerKind>,
    reviewers: ReviewerSet,
    default_urgency: Urgency,
    on_timeout: TimeoutBehavior,
    decision_validity: Option<Duration>,
}

enum ReviewerSet {
    AnyOf(Vec<Identity>),
    AllOf(Vec<Identity>),
    Quorum { members: Vec<Identity>, n: u32 },
    Role(NamespacedName),
    Dynamic(CfpUri),
}

enum TimeoutBehavior {
    AutoReject,
    AutoApprove,                       // 危险,需明确声明
    Escalate(Identity),
    Continue,
}
```

### 10.5 与现有 Hook 系统的关系

L5 协议设计参考并兼容现有 Agent Hook 系统(如 Claude Code 的 PreToolUse / PostToolUse / Stop hooks):

| Hook 系统 | CFP L5 对应 | CFP 的增强 |
|------|------|------|
| PreToolUse | `Propose { trigger: BeforeAction }` | 提议带完整 rationale,不只是工具名 |
| PostToolUse | `Review { trigger: AfterAction }` | 可触发级联(撤回、回滚) |
| UserPromptSubmit | `Propose { trigger: Custom("user-input") }` | 用户输入也是被审查对象 |
| Stop | `Review { trigger: StateTransition(session-end) }` | 触发承诺履约验证 |
| SubagentStop | 嵌套 ReviewChain | 子 Agent 审查链路嵌套 |

**实现策略**:现有 Agent 平台的 Hook 机制可以作为 CFP L5 的本地实现——hook 接收事件、生成 CFP L5 对象、写入 ReviewChain、返回 verdict。

### 10.6 双向流模型

L5 假设传输支持双向流(HTTP/3、WebSocket、stdio)。客户端和服务端角色对称——任何一方都可以发起 Challenge 或 Subscribe。

---

## 11. L6: Presentation View Layer 展示视图层 (v0.3 重构)

### 11.1 Runtime Family 概念

CFP v0.3 重构 L6 为 **Rendering Runtime Family** 模型。L6 不是"视图格式枚举",而是"运行时家族 + 视图绑定"。

```
L6 Presentation View
│
├── Web Runtime Family
│   ├── HTML View
│   ├── React Artifact View
│   ├── Vue View
│   └── 其他 Web 框架
│
├── Native AI Runtime Family    ← v0.3 新增重点
│   ├── Makepad Live DSL View
│   ├── Splash Animation View
│   └── 未来其他 Native UI Runtime
│
├── Document Runtime Family
│   ├── Markdown View
│   ├── PDF View
│   └── DOCX View
│
└── Machine Runtime Family
    ├── Agent View (CBOR/JSON)
    ├── Structured Data View
    └── 其他机器消费格式
```

**核心架构判断**:HTML 和 Makepad 在 L6 是**平起平坐的运行时分支**。HTML 是 Web Runtime Family 的代表,Makepad 是 Native AI Runtime Family 的代表。两者服务不同的目标场景,但在 CFP 协议中地位等同。

### 11.2 View 对象

```rust
struct View {
    id: CfpUri,
    kind: ViewKind,
    runtime_family: RuntimeFamily,
    target: CfpUri,
    options: Map<String, Value>,
}

enum RuntimeFamily {
    Web,
    NativeAi,
    Document,
    Machine,
    Custom(NamespacedName),
}

enum ViewKind {
    // Web Runtime Family
    Html,
    ReactArtifact,
    VueComponent,

    // Native AI Runtime Family
    MakepadLiveDsl,
    SplashAnimation,

    // Document Runtime Family
    Markdown,
    Pdf,
    Docx,

    // Machine Runtime Family
    AgentView,
    StructuredData,

    // 通用
    InteractiveTimeline,
    SpatialAnnotation,
    AppBundle,

    Custom(NamespacedName),
}
```

### 11.3 视图层的演化与中立性

L6 的视图类型不是固定集合。CFP 不绑定任何具体渲染技术。

已确认的视图历史演化路径:

```
Raw Text → Markdown → HTML → Dynamic UI (React/Vue) →
                              Native Generated UI (Makepad/Splash) →
                              Neural Generated UI (Diffusion)
```

每个阶段提供更高的视觉带宽和交互能力。**CFP 协议在所有这些渲染选择中保持中立**——L0–L4 的真理之源不变。

行业当前对"HTML vs Markdown 哪个更适合 AI"的争论,实际上是渲染层选择的争论,不是协议层争论。CFP 的立场是:

- **知识本体**:使用 Markdown 作为 L0–L1 的轻量级输入语法
- **Web 临时消费**:使用 HTML / Dynamic Web UI
- **Native AI 消费**:使用 Makepad / Splash / 其他 native UI runtime
- **AI 间通信**:使用 CBOR / Agent View

四种使用方式可以同时存在,服务不同消费者。

### 11.4 标准视图渲染规则

#### 11.4.1 Web Runtime Family

**HTML View**:
- 每个 Claim 渲染为带 `data-cfp-claim-id` 属性的元素
- 可被 CSS 选择器和 JS 工具识别
- 状态通过 ARIA 属性表达

**React Artifact View**:
- 嵌入 CFP 元信息作为 `<script type="application/cfp+json">` 块
- 组件状态绑定到 CFP 对象的字段
- 修改触发 L5 Interaction Event

#### 11.4.2 Native AI Runtime Family

**Makepad Live DSL View**:
- CFP 对象映射为 Makepad Live DSL 节点
- 支持热重载(Live DSL 特性天然契合 generated UI 场景)
- GPU 加速渲染,启动延迟显著低于 Web Runtime
- 直接访问 native capabilities(文件系统、硬件、传感器)

**关键优势**:
- 不依赖浏览器
- 更低延迟、更高响应性
- 类型安全(Rust 编译期保证)
- 适合端侧 AI 设备、桌面 Agent、嵌入式应用

#### 11.4.3 Document Runtime Family

**Markdown View**:
- Claim 的 `text` 字段作为正文
- Opinion 类型 **MUST** 加视觉区分
- Inference 类型 **SHOULD** 在脚注或折叠区显示 premises
- Commitment 渲染为带状态徽章的块

**PDF View**:
- 用于合规归档、打印分享
- 保留 CFP 元信息作为 PDF 附件或自定义元数据

#### 11.4.4 Machine Runtime Family

**Agent View**:
- 直接返回 CFP 对象图的结构化形式
- 跳过任何视觉装饰
- 通常用 CBOR 序列化

### 11.5 视图是派生的

**关键架构约束**:View **MUST NOT** 是权威数据源。修改 View 不影响底层 Claim 对象图。修改必须通过 L5 交互原语触发 L1 状态转换。

---

## 12. URI 寻址规范

### 12.1 URI 格式

```
cfp://<authority>/<object-id>[@<version>][#<anchor>]
```

`anchor` 部分 **SHOULD** 兼容 W3C Media Fragments URI:

```
cfp://example.com/video-001#t=12.3,18.7
cfp://example.com/img-001#xywh=420,680,310,160
```

### 12.2 解析规范

- `authority` 使用域名或 DID
- 同一 `authority` 下 `object-id` **MUST** 唯一
- `version` 缺省时指最新版本
- 实现 **SHOULD** 支持 content-addressed URI 作为不可变引用

---

## 13. 序列化与传输

### 13.1 序列化格式

CFP **MUST** 支持:

| 格式 | 用途 | 优先级 |
|------|------|------|
| **JSON** | 基线格式 | MUST |
| **CBOR** | Agent 间高效传输 | SHOULD |
| **YAML-flavored DSL** | 人类轻量书写 | MAY |

JSON 与 CBOR **MUST** 在语义上等价。

### 13.2 传输绑定

| 传输 | 适用场景 |
|------|------|
| HTTP/3 + JSON | Web 端 |
| HTTP/3 + CBOR | Agent ↔ Agent |
| WebSocket | 双向流、订阅推送 |
| stdio | 本地 Agent 工具集成 |
| 文件系统 | 文档存储,Git 友好 |

---

## 14. 安全考虑

### 14.1 内容真实性

- Claim、Source、Commitment **MAY** 携带签名
- 未签名内容 **MUST NOT** 被自动赋予高信任级别

### 14.2 承诺执行安全

- 默认权限 `RequiresConfirmation`
- 实现 **MUST** 提供权限审查 UI/日志
- 实现 **SHOULD** 支持 dry-run

### 14.3 审查不可绕过 (v0.3 新增)

根据原则 P15:

- 任何 Capability 为 `RequiresConfirmation` 的承诺,实现 **MUST** 等待 ReviewEvent
- **MUST NOT** 由实现自行决定跳过审查
- 超时行为 **MUST** 遵循 ReviewPolicy 中显式声明的 `on_timeout`
- 实现 **MUST** 持久化 ReviewChain,即使审查最终被 Cascade 取消

### 14.4 多模态防伪

- Media Asset 的 `hash` **MUST** 存在
- 实现 **SHOULD** 集成 C2PA 验证
- AI 生成媒体 **MUST** 通过 `authenticity.ai_generated` 显式标注

### 14.5 多模态隐私

- `CaptureContext` **MUST** 在传输前隐私脱敏
- 默认 `location_precision` **MUST** 不高于 `City`
- 提升精度 **MUST** 经过用户显式授权

### 14.6 说话人识别

- `MediaSegment.speaker_label` **MUST** 仅作为本地标签
- 真实身份绑定 **MUST** 通过独立 Claim 表达
- **MUST NOT** 在未授权情况下传输

---

## 15. 与现有标准的关系

| 标准 | CFP 中的角色 |
|------|------|
| **HTML** | L6 Web Runtime Family 的代表 |
| **React/Vue/Svelte** | L6 Web Runtime Family 的现代成员 |
| **Markdown** | L6 Document Runtime Family + L0–L1 输入降级语法 |
| **Makepad / Splash** | L6 Native AI Runtime Family 的参考实现 |
| **JSON / CBOR** | 主序列化格式 |
| **JSON Schema / TypeSpec** | Claim 类型 schema 定义 |
| **JSON-LD / RDF** | L0 可选语义图绑定 |
| **DID / VC** | Identity 与签名 |
| **W3C Media Fragments URI** | L2 Segment 引用语法 |
| **C2PA** | L2 多模态溯源 |
| **JPEG Trust** | L2 图像真实性凭证 |
| **EXIF / XMP** | L2 媒体元数据兼容 |
| **ActivityPub** | 跨实例订阅与传播(未来) |
| **OpenAPI / MCP** | L4 工具与能力定义 |
| **CRDT (Yjs, Automerge)** | L1 多方协作底层 |
| **Git** | L1 事件溯源实现选择 |
| **WASM** | L4 承诺履约的安全执行环境 |
| **Agent Hook Systems** (Claude Code, etc.) | L5 审查协议的本地实现 |

CFP 的贡献不是发明新基础设施,而是**定义这些基础设施之间的接口语义**。

---

## 16. 典型应用场景 (v0.3 重新聚焦)

### 16.1 场景一:Native AI Generated App —— CFP 的核心场景 (v0.3 重点)

#### 16.1.1 场景描述

用户用自然语言描述需求,Agent 生成完整应用,部署到 **native AI runtime**(典型如 Makepad host)。典型部署目标:

- 桌面 native AI 助手(macOS/Linux/Windows 原生)
- 端侧 AI 设备(M5Stack 类硬件、AI Pin、智能眼镜)
- 嵌入式 Agent(机器人控制器、IoT 设备)
- 高响应交互场景(实时数据可视化、动画驱动 UI)

#### 16.1.2 为什么是 Native 而非 Web

Web HTML Artifact 在 Web 上已被验证(参考 v0/Lovable/bolt.new 的成功)。但 Web Runtime 在以下场景失效:

- **延迟敏感**:浏览器启动 + DOM 渲染管线对实时交互过慢
- **硬件访问**:需要直接访问相机、传感器、本地文件、GPU
- **离线运行**:端侧设备可能无网络
- **资源受限**:嵌入式设备无法承载浏览器
- **类型安全**:对可靠性要求高的场景需要编译期保证

**Native AI Runtime(以 Makepad 为参考实现)** 在这些场景下提供与 Web Runtime 对等的能力:

- Rust 写的、GPU 加速的、跨平台原生 UI 框架
- Live DSL 支持热重载,与 generated UI 场景天然契合
- 无浏览器依赖,启动延迟低
- 直接访问 native capabilities
- 类型安全在编译期保证

#### 16.1.3 三个递进层次

| Level | 描述 | 状态 |
|------|------|------|
| Level 1 | 动态生成无状态 View(展示) | 已有 Web 工具,native 待发展 |
| Level 2 | 动态生成有状态 View(数据 + 交互) | 待标准化 |
| Level 3 | 独立 App(完整应用,可独立运行) | 早期阶段 |

#### 16.1.4 当前痛点

| 痛点 | 表现 |
|------|------|
| 设计意图丢失 | 生成代码三周后改不动 |
| 修改 = 重新生成 | 微调一个功能往往整体重写 |
| 不可信 | 用户不知道生成的 App 是否安全 |
| 不可问责 | App 出错时无责任链 |
| 孤岛 | 不同 Agent 生成的 App 互不兼容 |

#### 16.1.5 CFP 解决方案:App Bundle

Agent 生成 App 时输出完整的 **App Bundle**:

```rust
struct AppBundle {
    // 标识与版本
    id: CfpUri,
    version: SemVer,
    generated_by: Identity,
    generated_at: Timestamp,

    // 运行时绑定 (v0.3 新增核心字段)
    target_runtime: RuntimeBinding,

    // 设计意图链
    user_intent: CfpUri,                  // Intent Claim
    inferences: Vec<CfpUri>,              // 推理链
    decisions: Vec<CfpUri>,               // 关键决策与理由

    // 代码工件
    code_artifacts: Vec<CodeArtifact>,

    // 数据模型
    schemas: Vec<CfpUri>,
    invariants: Vec<CfpUri>,              // Invariant Commitments

    // 承诺清单
    commitments: Vec<CfpUri>,             // 安全、隐私、可用性承诺

    // 能力依赖
    required_capabilities: Vec<Capability>,

    // 演化历史
    parent_version: Option<CfpUri>,
    change_log: Vec<CfpUri>,
}

// v0.3 新增:Runtime Binding
enum RuntimeBinding {
    Web {
        framework: WebFramework,
        sandbox: SandboxKind,
    },
    NativeAi {
        platform: NativePlatform,         // Makepad / 其他
        version: SemVer,
        live_dsl: bool,
        capabilities_required: Vec<NativeCapability>,
    },
    Document {
        format: DocumentFormat,
    },
    Server {
        language: String,
        framework: Option<String>,
    },
    Custom(NamespacedName, Value),
}
```

#### 16.1.6 跨 Runtime 渲染:协议层的核心价值

```
用户描述需求
    ↓
Agent 推理产生 L0–L4 对象图(共享真理之源)
    │
    ├──→ Bundle (Web Runtime: HTML/React)        ← Web 部署
    ├──→ Bundle (Native AI Runtime: Makepad)     ← 端侧 AI 部署
    └──→ Bundle (Document Runtime: PDF Spec)     ← 规范归档
```

**同一份设计意图、同一份承诺契约、同一份决策树,但产出不同 runtime 的 App Bundle**。这是协议的核心价值兑现。

#### 16.1.7 价值演示

**场景**:用户让 Agent 生成"读书进度跟踪器"。

**没有 CFP**:
- 200 行代码,三周后改不动
- 修改需要重新生成
- 无法追溯为何当初这么设计

**有 CFP**:

```
AppBundle "reading-tracker-v1"
target_runtime: NativeAi { platform: Makepad, ... }
├── Intent: "用户想跟踪读书进度,记录页数"
├── Inference: "因此核心数据模型应该是 BookRecord"
├── Decision: "状态管理用 Makepad live state 而非外部 store"
├── Schema: BookRecord { title, pages_read, total_pages }
├── Invariant: pages_read MUST NOT exceed total_pages
├── Privacy Commitment: 数据仅本地存储
└── Code: Makepad Live DSL files
```

三周后修改时,Agent 看到完整设计决策树,**增量补丁**而非重写。

#### 16.1.8 价值评估

| 维度 | 当前方式 | CFP 方式 | 提升量级 |
|------|------|------|------|
| 三周后修改成本 | 接近重新生成 | 增量补丁 | 5–10× |
| 用户信任度 | 黑盒 | 可查看承诺清单 | 质变 |
| 错误追溯能力 | 几乎不可能 | 完整事件链 | 质变 |
| 跨 Runtime 复用 | 不可能 | 同一对象图多目标渲染 | 质变 |
| 合规审计 | 极难 | 协议级支持 | 质变 |

### 16.2 场景二:Web Agent2App (与 HTML Artifact 浪潮接驳)

适用工具:Vercel v0、bolt.new、Lovable、Claude Artifacts、Replit Agent。

CFP 为这些工具提供:
- 嵌入式 CFP 元信息(`<script type="application/cfp+json">`)
- 跨 Session 的设计决策保留
- HTML Artifact 间的可组合性

价值:让 Web HTML Artifact 从"一次性产物"升级为"可演化资产"。

### 16.3 场景三:多 Agent 协作编码

通过 L0 类型化主张 + L1 状态流提供共享认知层。

当一个 Agent 修改 API 时,所有依赖该 API 的 Claim 自动进入 `Challenged` 状态,订阅它们的其他 Agent 收到通知。无需人类协调员。

价值评估:从"不可能"到"可能"。

### 16.4 场景四:严肃技术写作

CFP 强制区分 Fact / Opinion / Inference,提供来源链与新鲜度检查,让 AI 协作内容归属清晰。

价值:在 AI 内容充斥时代,"可审计的技术写作"成为创作者核心差异化护城河。

### 16.5 场景五:Agent 调用 App API (与 MCP 协同)

API 调用携带 CFP 元信息(initiated_by、part_of_commitment、rationale),让 App 理解调用上下文。

价值:补充 MCP,不是替代。

### 16.6 场景价值密度排序

| 排名 | 应用场景 | 价值密度 | CFP 主要价值层 |
|------|------|------|------|
| 1 | **Native AI Generated App** | **极高(质变)** | L0, L1, L4, L6(Native) |
| 2 | 多 Agent 协作编码 | 极高(质变)| L0, L1, L5 |
| 3 | Web Agent2App | 高(质变)| L0, L1, L4, L6(Web) |
| 4 | 严肃技术写作 | 高 | L0, L1, L3 |
| 5 | AI 协作写作 | 中高 | L0, L3 |
| 6 | Agent 调 App API | 中 | L4 |

### 16.7 CFP 不适合的场景

为避免协议过度宣称:

1. 延迟敏感的实时场景(高频交易、实时游戏)
2. 隐私优先且无需审计的场景(私人对话、心理咨询)
3. 创意发散场景(诗歌、小说草稿)
4. 简单单次问答
5. 协议未达成生态共识前的早期产品

---

## 17. CFP 与 Agent 系统架构 (v0.3 新增)

CFP 在 Agent 系统中的位置容易被混淆。本节明确 CFP 与 Session、Memory 等概念的关系。

### 17.1 三者的本体定义

**Session** 是 Agent 运行时的边界:
- 一次对话的时间和进程边界
- 短期、隔离
- 不是数据格式,是运行时容器

**Memory** 是跨 Session 持续存在的知识:
- 长期、跨 Session、可查询
- 主流实现:向量库 RAG、键值存储、知识图谱、事件日志
- Memory 是**问题**,不是协议

**CFP** 是内容协议:
- 数据格式 + 语义模型 + 交互协议
- 不是运行时(不像 Session)
- 不是存储引擎(不像向量库)
- CFP 是**协议**,不是问题

### 17.2 三者正交关系

```
                  Memory (长期/跨 Session)
                       ↑
                       │
        ╔══════════════╪══════════════╗
        ║   CFP 数据格式可用于这整个空间   ║
        ╚══════════════╪══════════════╝
                       │
                       ↓
                  Ephemeral (短期/Session 内)

        ←──────────────────────────────→
   Unstructured (free-form)    Structured (CFP)
```

CFP 是正交于 Session/Memory 的"内容格式维度"。

### 17.3 CFP 在 Session 内的角色

CFP 可以把对话历史升级为结构化对象图:

```
Session 状态(CFP 增强):
├── Intent Claim: "用户原始需求"
├── Decision Claim: "Agent 决策"
├── Commitment: "Agent 承诺"
├── Inference Chain: 完整推理链
└── Modification Claim: "用户修订"
```

好处:
1. Agent 不需要每次重新解析对话历史
2. 多轮对话的"理由树"可见
3. 会话内修订可追溯
4. Session 结束时可无损序列化

**适用场景**:多轮复杂任务、涉及多工具调用、Agent 生成代码/App。

**不适用场景**:单轮问答、闲聊、延迟敏感。

### 17.4 CFP 作为 Memory 数据格式

主流 Memory 实现把记忆当作"文本块"。这有几个问题:无时间语义、无信任语义、无更新机制、无承诺记录、无溯源。

CFP-based Memory 的核心价值:

```
用户的长期 CFP Memory Graph
├── Fact Claim (state: Superseded by fact-007)
├── Fact Claim (state: Superseded by fact-019)
└── Fact Claim (state: Verified)    ← 当前真相
```

下次查询时,**直接查询 state: Verified 的 Claim**,不是 RAG 检索多段文本让 LLM 猜。

### 17.5 推荐架构:三层 Memory

```
┌─────────────────────────────────────────┐
│  L3: Application Memory                  │
│  CFP Claim Graph (typed, stateful)      │
├─────────────────────────────────────────┤
│  L2: Episodic Memory                    │
│  CFP Event Log (per Session)            │
├─────────────────────────────────────────┤
│  L1: Semantic Index                     │
│  Vector Embeddings of CFP Claims        │
└─────────────────────────────────────────┘
```

CFP 不替代向量库——向量库继续承担相似度召回。但向量库索引的对象从"原始文本块"升级为"CFP Claim 对象"。

### 17.6 工程现实

短期内 CFP-Memory 是 add-on 而非 replacement:

- 主流 Memory 系统继续用 RAG 处理普通文本
- 关键决策、承诺、用户偏好用 CFP 表达(少量但高价值)
- 两者通过 CFP 的 Source 链接互通

长期看,如果 CFP 在 Agent2App 等场景证明价值,Memory 系统会逐步向 CFP 模型迁移。

---

## 18. MVP 实施路径

### 18.1 v0.3 (本文档)

**范围**:协议草案 v0.3,加入审查协议、Runtime Family、Native AI 场景重点。

**交付物**:
- 本规范文档
- JSON Schema(核心对象)
- 完整示例文档(附录 A)

**目标**:邀请社区讨论,特别是 Native AI 与 Agent2App 方向的实践者。

### 18.2 v0.4: Native AI Reference Implementation

**范围**:L0 + L1 + L4 + L6(Native AI Runtime)的最小可用实现,演示 Native AI Generated App。

**交付物**:
- Rust 实现的 `claimflow` 核心库
- **`claimflow-makepad` 适配器**:CFP 对象图 ↔ Makepad Live DSL
- Native AI Artifact Demo:LLM 输出 → CFP 对象图 → Makepad 应用热重载
- 命令行工具

**目标**:让 Native AI 开发者能 30 分钟上手。

### 18.3 v0.5: Multimodal + Web Bridge

**范围**:完整 L2 + L6 Web Runtime 适配。

**交付物**:
- 多模态证据库(C2PA 集成)
- Web Runtime 适配器(HTML Artifact + CFP 嵌入元信息)
- Markdown ↔ CFP 双向桥接

### 18.4 v0.6: Commitment & Review Runtime

**范围**:完整 L4 + L5 实现。

**交付物**:
- 承诺执行运行时
- ReviewChain 持久化与查询
- 与 Claude Code Hooks / 其他 Agent Hook 系统的桥接

### 18.5 v1.0: 标准化候选

**范围**:覆盖全部七层,至少 3 个独立实现通过互操作性测试。

**目标**:至少一个真实生产案例(建议在 Native AI Generated App 方向)。

---

## 19. 变更历史

| 版本 | 日期 | 主要变化 |
|------|------|------|
| 0.1 | 2026-05-11 | 初稿。六层架构,邀请社区讨论。 |
| 0.2 | 2026-05-11 | 新增多模态层 L2,协议栈升级到 7 层。新增应用场景章节,Agent2App 作为杀手级用例。强化安全考虑。新增 P11 P12。 |
| 0.3 | 2026-05-11 | **明确 Core Layer(L0–L4 AI 优先)与 Interface Layer(L5–L6 人类回路)分裂**(新增 4.4 节)。**L5 大幅扩展**,加入审查协议原语(Propose/Review/Defer/Withdraw)和 ReviewChain 一等对象。**L6 重构为 Runtime Family 模型**(Web / Native AI / Document / Machine 四大族,HTML 与 Makepad 平起平坐)。**Agent2App 重新定位**,Native AI Generated App 上升为核心场景。**新增 CFP 与 Agent 系统架构关系章节**(第 17 节),澄清与 Session/Memory 的正交关系。**新增 P13–P15**:核心层 AI 优先 / 界面层人类回路、Runtime 中立性、人类回路是协议级承诺。 |

---

## 20. 待解决问题 (Open Issues)

继承 v0.1–v0.2 的 OI-1 至 OI-13,新增以下问题:

### OI-14: Review Pattern Library

标准化常见审查模式(双因素审查、quorum 审查、级联审查)。是否需要 CFP-0003 副规范专门定义?

### OI-15: 跨 Runtime 的对象图等价性

同一份 L0–L4 对象图渲染到 Web 和 Native 时,如何保证功能等价?需要定义"等价性测试套件"。

### OI-16: Native AI Runtime 标准化

Makepad 是 v0.3 的参考实现,但 Native AI Runtime Family 应该有更多成员吗?(Tauri、Slint、Iced 等)如何标准化它们的 CFP 适配?

### OI-17: 审查链路的隐私管理

ReviewChain 持久化所有审查决策,但某些场景需要审查内容保密(医疗、法律)。需要定义访问控制策略。

### OI-18: Memory 与 CFP 的统一查询语言

CFP 对象图 + 向量召回 + 状态过滤,组合查询语义如何标准化?建议 CFP-0004 副规范专门处理。

### OI-19: 审查 Agent 经济

高风险决策可以外包给专门审查 Agent,形成"审查者市场"。如何在协议层定义审查信誉、审查质量、跨组织审查互操作?

### OI-20: 边缘设备的 CFP 实现

端侧 AI 设备(M5Stack 类硬件)资源受限。CFP 是否需要 "lite 子集"规范?

---

## 21. 致谢

CFP v0.3 在 v0.2 基础上演化,本次更新的多个关键设计来自社区讨论:

- **L5 审查协议**的设计灵感来自 Claude Code Hooks 模式和社区对"人机回路"的讨论
- **L6 Runtime Family 概念**来自对"HTML vs Makepad"在不同部署场景下角色的辨析
- **Native AI Generated App** 作为核心场景的提升,来自对 Web HTML Artifact 浪潮的反思——native AI 时代需要对应的 generated UI 范式
- **Core Layer vs Interface Layer 的分裂**,来自对"七层都是 AI 优先吗"这一直击协议内部一致性问题的诚实回应
- **Session/Memory 关系澄清**,来自对 CFP 在 Agent 系统架构中位置的清晰化需求

继续感谢:

- **Tim Berners-Lee 的 Semantic Web** 与 RDF 三元组
- **Notion、Roam Research、Obsidian** 的块结构内容模型
- **Andrej Karpathy 的 LLM Wiki / LLM 知识库范式** 与 GUI 演化预测
- **Suryansh Tiwari 的 "Unreasonable Effectiveness of HTML"** 信息图——精准描绘了 HTML Artifact 浪潮
- **Makepad 团队 (Rik Arends 等)**——为 Native AI generated UI 提供了关键的运行时基础
- **Git 的事件溯源与内容寻址**
- **C2PA / JPEG Trust**——多模态溯源标准
- **W3C Media Fragments Working Group**——精确媒体引用语法
- **MCP (Model Context Protocol) / Claude Code Hooks**——Agent 协作的早期实践
- **Anthropic Artifacts / Vercel v0 / Replit Agent / Lovable / bolt.new**——Agent2App 范式的市场验证者
- **agent-spec**——L4 承诺层与领域规范分层的来源

---

## 22. 参考文献

[RFC 2119] Bradner, S., "Key words for use in RFCs to Indicate Requirement Levels", BCP 14, RFC 2119, March 1997.

[RFC 8949] Bormann, C. and P. Hoffman, "Concise Binary Object Representation (CBOR)", STD 94, RFC 8949, December 2020.

[JSON-LD] Sporny, M., et al., "JSON-LD 1.1", W3C Recommendation, July 2020.

[DID] Sporny, M., et al., "Decentralized Identifiers (DIDs) v1.0", W3C Recommendation, July 2022.

[C2PA] Coalition for Content Provenance and Authenticity, "C2PA Technical Specification", 2024.

[MediaFrag] Troncy, R., et al., "Media Fragments URI 1.0", W3C Recommendation, September 2012.

[JPEG Trust] ISO/IEC 21617-1:2024, "JPEG Trust", 2024.

[MCP] Anthropic, "Model Context Protocol Specification", 2024.

[Makepad] Arends, R., et al., "Makepad: Rust native UI framework with Live DSL", <https://makepad.dev>

[Karpathy-GUI] Karpathy, A., "On the future of LLM GUI", X post, 2025.

[Karpathy-LLMWiki] Karpathy, A., "LLM Knowledge Bases", GitHub Gist, April 2026.

---

## 附录 A: 完整示例

### A.1 Native AI Generated App Bundle (Makepad 目标)

```
@app reading-tracker-v1
generated_by: did:agent:claude-2026-05
generated_at: 2026-05-11T05:00:00+02:00
parent_version: null

target_runtime: native_ai
  platform: makepad
  version: 0.7.0
  live_dsl: true
  capabilities_required:
    - filesystem.read.local
    - filesystem.write.local

#claim intent001 intent issuer=did:user:alex
text: 用户想跟踪个人读书进度,记录已读页数与总页数。

#claim inference001 inference premises=[intent001] confidence=0.95
text: 核心数据模型应为 BookRecord(title, pages_read, total_pages)。
method: deductive

#claim decision001 decision premises=[inference001] confidence=0.9
text: UI 使用 Makepad Live DSL,状态管理用 makepad widget state。
rationale: native 启动延迟低,Live DSL 支持热重载,适合开发期快速迭代。

#claim schema001 fact verifiability=internal
text: BookRecord 数据模型
structured:
  fields:
    - name: title
      type: string
    - name: pages_read
      type: integer
    - name: total_pages
      type: integer

#commitment inv001 invariant
obligation: maintain("pages_read <= total_pages")
verification: automated:makepad-input-validator
permission_level: auto-executable
review_policy:
  triggers: [BeforeAction]
  reviewers: AnyOf(did:user:alex)
  on_timeout: AutoReject
status: active

#commitment priv001 privacy
obligation: refrain("any-network-transmission")
verification: automated:makepad-network-monitor
permission_level: forbidden
status: active

#code-artifact app.rs
language: rust-makepad-live
hash: sha256:codehash...
```

### A.2 跨 Runtime 渲染示例

同一份 reading-tracker-v1 对象图可绑定不同 Runtime:

**Web Runtime 版本**:
```
target_runtime: web
  framework: react
  sandbox: browser
code_artifact: app.tsx
```

**Document Runtime 版本**(用于规范归档):
```
target_runtime: document
  format: pdf
code_artifact: spec.pdf
```

L0–L4 完全相同,只有 L6 的 runtime binding 和 code_artifact 不同。

---

## 附录 B: JSON Schema

完整 JSON Schema 待发布于 `https://cfp.spec/v0.3/schema.json`(草案)。v0.3 新增对象包括:

- `ProposeEvent`、`ReviewEvent`、`ReviewChain`
- `ReviewPolicy`、`ReviewerSet`、`TimeoutBehavior`
- `RuntimeBinding`、`RuntimeFamily`(枚举升级)
- `AppBundle` 增强字段

---

**本文档对讨论开放。所有设计决策接受挑战。**

提交反馈:
- GitHub Issues:(待建仓库)
- 邮件:(待补充)
- 微博/X:标签 `#CFP-Protocol`
