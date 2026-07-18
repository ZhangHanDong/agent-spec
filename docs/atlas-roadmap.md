# rust-atlas Roadmap：从"精确的 Rust 语法图"到"多语言 + 语义解析的符号图"

> 状态基线：**Phase 0（`afca280`）与 Phase 1（`bb47849`，合入 PR #5）均已完成**；
> 本文剩余部分（Phase 2–4）是前瞻规划。本文综合了对 `crates/rust-atlas/src/lib.rs`
> 的源码审查、在真实 workspace（最新 grok-build 124d85b：68,638 节点 / agent-spec
> 自举 ~2,300 节点）上的实测审计，以及对 tree-sitter / rust-analyzer 两条演进路线的
> 取舍分析。
>
> 注：下文各处 `lib.rs:NNN` 行号锚定 Phase 1 完成前的快照，仅作定位线索；Phase 1
> 大改后行号已漂移，以符号名为准。
>
> 落地方式：本文件是**设计文档**。每个 Phase 的可交付项应切成 `specs/` 下的
> `.spec.md` 合约（沿用 `task-atlas-*.spec.md` 约定）再交付实现。

---

## 0. 指导原则（不可动摇的约束）

1. **分层 provenance 不变**。schema 里的 `Provenance { Syn, Scip, Mir }`
   （lib.rs:68）已经编码了三层设计意图：语法基线 / 语义 overlay / 全程序分析。
   所有演进都往这三层里填，而非另起炉灶。
2. **语法基线永远离线可用**。基线层（当前 syn，未来可加 tree-sitter）必须对**任意
   文本**、**不可编译的项目**都能产出结果。语义层是 opt-in 的增强，不能成为硬依赖。
3. **schema 向后兼容**。任何字段变更走 `SCHEMA_VERSION` 递增 + `#[serde(default)]`，
   旧 shard 必须仍可反序列化（现有做法，保持）。
4. **不变量即测试**。`validate_graph`（lib.rs:568）在构建期强制"节点 id 唯一 +
   resolved 边端点存在"。新增边类型/解析路径都必须纳入该校验，且配回归测试。
5. **实测优先**。每个 Phase 的验收都要在真实 workspace 上跑数字（解析率、节点/边
   计数、查询命中），而非只看单测通过。

---

## 1. 当前状态

### 1.1 已修复（Phase 0，`afca280`）— 5 项原始缺陷，均已实证

| 缺陷 | 修法 | 验收证据 |
|---|---|---|
| 虚拟 workspace 塌成 `crate::crates::…::src` | `cargo metadata` 真布局（`ProjectLayout::discover` lib.rs:894） | grok-build 66,993 节点 0 坏 id |
| impl/泛型 id 碰撞 | `declaration_id`+计数器 + `validate_graph` 强制唯一 | build 成功即证全图唯一 |
| 签名抓到 doc/属性 | `normalized_tokens(&sig)` + 独立 `doc` 字段 | 签名无泄漏，352 节点带 doc |
| 悬空边伪装成真边 | `EdgeResolution` + `resolve_syn_edges` 事后解析 | 98.1% resolved / 其余诚实标注 |
| SCIP 状态不一致 | 无 `--scip` 即 `remove_scip_edges` 清 overlay | 回归测试覆盖 |

### 1.2 Phase 1 已修复（`bb47849`）— 审计暴露的 5 项新问题，均已实证

| ID | 问题 | 结果 |
|---|---|---|
| **A** | `impls <内部Trait>` 返回空：裸名 trait 在 syn 层无法解析 + `impls()` 只认 resolved id | 裸名唯一后缀解析 + impls 兜底；`impls SpecLinter` 0→27；impls-trait resolved 3/51→41/51 |
| **B** | 布局过度收集：被 `exclude` 的 fixture crate + `build_script_build` 污染图 | 尊重 `exclude` + 跳过 custom-build + walk 层过滤；污染 33→0 |
| **C** | 完整性缺口：`static`/`union`/`TraitAlias`/关联 `const`·`type` 不入图 | 补齐 item 类型 + 新 `NodeKind`；最新 grok-build 图含 static 339、type-alias 549 |
| **D** | struct/enum 签名嵌入整个字段体（最长 1122 字符） | 声明头签名；struct 签名最长 1122→47 |
| **E** | 死字段 `Capability.scip_index`；`refs` 无 scip 恒空未文档化 | 删死字段 + refs 文档化 |
| **F** | `--code .`（CLI 默认值）崩溃：cargo metadata 绝对路径 vs 相对遍历路径 | `build`/`check` 入口 canonicalize；默认命令恢复可用 |

> 遗留（Phase 1 未完全覆盖，见 §9）：非 `exclude` 的 stray 文件仍被兜到 host crate
> 命名空间；`crate::Tool` 式 re-export 路径不解析（留给 Phase 2 SCIP）。

---

## 2. 目标架构：一条谱系，三个 provenance 层

```
             语法·多语言            语法·Rust            语义·Rust             全程序
  ┌──────────────────────┐ ┌────────────────┐ ┌────────────────────┐ ┌──────────────┐
  │     tree-sitter      │ │      syn       │ │ rust-analyzer→SCIP │ │  rustc MIR   │
  │ 容错/增量/多语言/近似 │ │ Rust结构化/精确 │ │ 名称解析/宏展开/类型 │ │  调用图/数据流 │
  └──────────┬───────────┘ └───────┬────────┘ └─────────┬──────────┘ └──────┬───────┘
             └──── 基线层(Provenance::Syn) ────┘         Provenance::Scip     Provenance::Mir
                    离线·always-on                        opt-in·精确          future
```

- **tree-sitter 与 syn 是同一层的两个后端**（多语言广度 vs Rust 精度），可共存。
- **rust-analyzer 不是 parser，是 resolver**：它经 `rust-analyzer scip` 产 SCIP，
  喂给已有的 `overlay_scip`。这是修复 **finding A** 的正解，也是 `Calls`/`UsesType`
  （lib.rs:54-55，当前从不生成）的数据来源。
- **MIR 层**（`Provenance::Mir`，已在枚举里预留）留给未来的调用图/数据流。

---

## 3. Phase 1 — 巩固 syn 基线层（无新依赖，纯正确性 / 可用性）✅ 已完成（`bb47849`）

**目标**：把审计的 A（廉价部分）、B、C、D、E 清掉，让**离线 Rust 图**本身达到"可信、
完整、可导航"。全部不引入新依赖、不需要可编译项目。**下列子节记录各项的做法与
验收，均已实现并配回归测试（18→23 测试）。**

### P1-A 裸名唯一后缀解析 + `impls()` 兜底 — **最高 ROI**
- `resolve_syn_edges`（lib.rs:511）：当 `target_text` 无 `::` 且精确匹配失败时，
  尝试**唯一后缀匹配**——若全图恰有一个 `node.symbol` 以 `::{target_text}` 结尾，
  解析过去、标 `Resolved`；多于一个则保持 `Unresolved`（不猜）。
  - 预建一个 `last_segment -> [symbol]` 索引，O(1) 查、避免全表扫。
- `impls()`（lib.rs:432）：除按 resolved node id 匹配外，**再对 `target_text` 的
  末段做匹配**，兜住仍未解析的边（并在输出里标注 `resolution`，让调用方知道是近似）。
- **验收**：`atlas impls SpecLinter` 从 0 → 召回其实现者（agent-spec 自举图中
  应 ≥ 27）；impls-trait 的 resolved 比例从 ~6% 显著上升。
- **风险**：后缀匹配对重名 trait 会退回 Unresolved（可接受，宁缺毋误）。真正跨 crate
  的精确解析留给 Phase 2 的 SCIP。

### P1-B 尊重 workspace 边界，过滤噪声 target
- `ProjectLayout::discover`（lib.rs:894）/`nested_workspace_manifests`（lib.rs:702）：
  - 尊重根 `Cargo.toml` 的 `exclude`（不要经嵌套 workspace 把被排除的 fixture 捞回）。
  - 跳过 `custom-build` / `build_script_build` target（`target.kind` 含 `custom-build`）。
  - 可选：提供 `--include-tests` / `--include-fixtures` 开关，默认只索引 lib/bin。
- **验收**：agent-spec 自举图不再出现 `atlas_basic` / `requirements_noteapp` /
  `build_script_build` 命名空间；孤儿节点归零。

### P1-C 补齐 item 类型
- `extract_items`（lib.rs:1387）新增匹配臂：`Item::Static`、`Item::Union`、
  `Item::TraitAlias`；在 trait/impl 体内补 `TraitItem::Const`/`Type`、
  `ImplItem::Const`/`Type`（当前只提 `Fn`）。
- 需要新 `NodeKind`（`Static`、`Union`、`TraitAlias`、`AssocConst`、`AssocType`）→
  `SCHEMA_VERSION` 递增。
- **验收**：源码里的 `static`/关联项在图中可查；`NodeKind` 覆盖率测试。

### P1-D 声明头签名
- struct/enum 签名只渲染到 `{` 之前（`pub struct SpecMeta`），字段/变体不进签名。
- **验收**：struct 签名长度中位/最长大幅下降；shard 体积下降。

### P1-E 清理与文档化
- 删除死字段 `Capability.scip_index`（lib.rs:122）——**Phase 2 会以"复用上次 index"
  的形式正式引入并真正 wire 上**，届时再加回带真实语义的版本。
- `refs`（lib.rs:395）：无 scip 时在输出里返回一条 `note`/warning 说明"无语义层，
  引用为空"，避免"静默空 = 没有引用"的误读。
- 可选：`line_start` 指向声明关键字行（排除 doc/属性），`line_end` 不变。

**Phase 1 交付物**：`specs/task-atlas-syn-hardening.spec.md`（含 A/B/C/D/E 的 BDD
场景）+ 实现 PR + 每项回归测试 + 一份"agent-spec 自举图前后对比"数字。

---

## 4. Phase 2 — 语义 overlay：rust-analyzer 经 SCIP 真正接上（修复 finding A 的根）

**目标**：把 `Provenance::Scip` 层从"仅 occurrence→References"升级为"完整语义解析"，
让 `impls`/`refs`/`calls` 对**内部与跨 crate** 都答对，并让宏生成的符号可见。

### P2-1 SCIP 生成流水线
- 新增 `atlas scip-gen`（或 build 内置）：检测 `rust-analyzer`，对 code_root 跑
  `rust-analyzer scip .` 产 `index.scip`（protobuf）或 JSON；缓存到 graph_dir、
  按内容哈希增量。
- 优雅降级：无 `rust-analyzer` / 项目不可编译 → 明确 warning + 退回纯 syn，**不报错**。
- 文档化前置：需可编译项目、匹配的 toolchain、proc-macro server。

### P2-2 扩展 `overlay_scip` 消费"关系"而非仅 occurrence
- 当前 `overlay_scip` 只读 `symbol_roles`（def/ref）产 `References`。**扩展**为：
  - 读 SCIP 的 **implementation relationships**，把 syn 层的 `ImplsTrait`/`ImplFor`
    边的 `to` 从"近似 target_text"**重写为 resolved node id**（`resolution=Resolved`,
    `provenance` 升级标记）。这才真正修复 **finding A**（Phase 1 的后缀匹配是廉价近似，
    此处是精确解）。
  - 产 `Calls` 边（方法/函数调用）——填上 lib.rs:54 那个从不生成的枚举。
  - 产 `UsesType` 边（字段/签名/局部的类型引用）——填上 lib.rs:55。
  - 复用已有的 `containing_node`（按 range 找最内层节点）做 SCIP symbol → atlas
    node id 的映射。
- **宏展开**：rust-analyzer 的 SCIP 覆盖 derive/宏生成的 impl（如
  `#[derive(Serialize)]`→`impl Serialize`），overlay 后这些 impl 变可见——这是
  syn/tree-sitter 永远给不了的完整性。

### P2-3 SCIP 跨增量刷新存活（正式实现被删的 `scip_index`）
- 把上次使用的 SCIP index 路径 + 指纹记进 `Capability`（真正 set/read）。
- `refresh`（lib.rs:609）/增量 `build`：若有记录的 index 且仍新鲜，**自动 re-overlay**
  而非 `remove_scip_edges` 清掉。使 `Provenance::Scip` 在编辑后稳定存活。
- 这解决了 Phase 0 遗留的"自动 refresh 会 purge scip"设计缺口。

**Phase 2 交付物**：`specs/task-atlas-scip-semantic.spec.md` + 实现 + 实测数字
（agent-spec：impls-trait resolved 比例、`refs`/`calls` 边数从 0→N、宏生成 impl 计数）。

---

## 5. Phase 3 — 多语言基线（tree-sitter）

**目标**：把基线层从"Rust-only（syn）"扩成"多语言"，覆盖混合仓库。

### P3-1 后端抽象
- 抽 `trait LanguageBackend { fn extract(&self, unit, source) -> (Vec<Node>, Vec<Edge>); }`。
  syn 成为 `RustSynBackend`（保留 Phase 1/2 全部精度）。
- `extract_shard`（lib.rs:1319）按文件语言分派到对应 backend。

### P3-2 tree-sitter 后端
- 为 Go / JS / TS / Python 各挂 `tree-sitter-*` grammar + `tags.scm` 查询
  （定义 + 近似引用），产 `Node`/`Edge`。用**声明式 `.scm` 查询**替代硬编码 match——
  也顺带让 Rust 侧未来"加一类符号 = 改查询"（呼应 finding C 的可扩展性）。
- 容错优势：tree-sitter 对残缺文件仍产部分树，消灭"整文件 unparsed"。
- 语言检测按扩展名；`walk_rs_files`（lib.rs:1073）泛化为 `walk_source_files`。

### P3-3 多语言语义层
- SCIP 本就是跨语言标准：Go=`scip-go`、TS=`scip-typescript`、Python=`scip-python`、
  Rust=`rust-analyzer`。同一个 `overlay_scip` 吃所有语言的 SCIP → 各语言都能有精确层。

**Phase 3 交付物**：`specs/task-atlas-polyglot.spec.md` + 多语言 fixture + 实现。
**排序说明**：Phase 3 独立于 2，可并行；但**建议 2 先行**——语义精度对 agent-spec
当前（Rust 为主）价值更高，且 finding A 是已暴露的实痛。

---

## 6. Phase 4 — 深化与硬化（future / 机会性）

- **MIR 层**（`Provenance::Mir`）：经 rustc MIR 或 charon 产精确调用图 / 数据流 /
  死代码。长期，重。
- **daemon / LSP 模式**：把 build 变常驻服务，这时 tree-sitter 的**子文件增量解析**
  才真正兑现（当前 CLI 批处理已在文件粒度拿到大部分增量收益）。
- **性能——增量已有、但有固定地板**。build 已按文件 blake3 哈希增量：只有内容变了
  的 `.rs` 才重新 syn 解析。实测最新 grok-build（68,638 节点）：首建 ~90s，**0 改动
  重跑仍 ~45s**——这 45s 是地板，来自每次都跑的三个 O(全图) 遍：哈希全部 2261 文件 +
  `resolve_syn_edges`（读全部 shard 建全局符号表、重解析所有边）+ `validate_graph`
  （读全部 shard 查唯一性/边端点）。优化方向：(1) `extract_shard` 用 rayon 并行；
  (2) 缓存 `cargo metadata`（当前每次 build shell 出）；(3) 把 resolve/validate 从
  "全图重算"缩到"只碰变化的符号及其引用方"（增量解析）；(4) SCIP 增量。
- **查询人体工学**：`impls`/`refs` 增加 `--provenance`/`--resolved-only` 过滤；
  `AmbiguousSymbol`（lib.rs:287）给出候选列表而非仅报错。

---

## 7. 排序、投入与风险

| Phase | 价值 | 投入 | 风险 | 新依赖 | 前置 |
|---|---|---|---|---|---|
| 1 syn 巩固 | 高（修实痛 A/B/C） | 小-中 | 低 | 无 | 无 |
| 2 SCIP 语义 | **很高**（根治 A + refs/calls + 宏） | 中 | 中（依赖 rust-analyzer 可用、可编译项目） | rust-analyzer（外部 CLI） | 建议在 1 后 |
| 3 tree-sitter 多语言 | 高（广度） | 大 | 中（grammar 维护、C 依赖） | tree-sitter + grammar crates | 独立，可并行 |
| 4 深化 | 中 | 大 | 高 | 视子项 | 2/3 之后 |

**推荐落地顺序**：**Phase 1 → Phase 2 →（Phase 3 视多语言需求）→ Phase 4**。
理由：先用零依赖把离线 Rust 图做到可信完整（1），再接上语义层根治最痛的解析缺口（2，
这也是 tree-sitter 给不了、必须靠 rust-analyzer 的部分），多语言广度（3）按需再上。

---

## 8. 附录：缺陷 → Phase → 代码触点 速查

| 缺陷/能力 | 落在 | 主要触点 |
|---|---|---|
| A 内部 trait impls 空 | 1.1（近似）+ 2.2（精确） | `resolve_syn_edges`:511、`impls`:432、`overlay_scip` |
| B fixture/build 污染 | 1.2 | `ProjectLayout::discover`:894、`nested_workspace_manifests`:702 |
| C 缺 static/union/关联项 | 1.3 | `extract_items`:1387、`NodeKind`:42 |
| D 签名膨胀 | 1.4 | `extract_items` struct/enum 臂 |
| E 死字段/refs 空/line_start | 1.5（清理）+ 2.3（scip_index 正式化） | `Capability`:122、`refs`:395 |
| Calls / UsesType 从不生成 | 2.2 | `EdgeKind`:54-55、`overlay_scip` |
| SCIP 增量存活 | 2.3 | `refresh`:609、`build`:176 |
| 多语言 | 3 | 新 `LanguageBackend`、`extract_shard`:1319、`walk_*`:1073 |
| 子文件增量 / 调用图 | 4 | daemon 模式、`Provenance::Mir` |

---

## 9. 已知遗留与边界（Phase 1 后仍在，如实记录）

在最新 grok-build（124d85b，68,638 节点，98.2% 边解析）上实测暴露：

- **re-export 路径不解析**。`impl Tool for X` 里 `Tool` 经 `use` 从 crate 根引入、
  写成 `crate::Tool`，而真正的 trait 节点在 `xai_tool_runtime::tool::Tool`（模块内）。
  带 `::` 故不走裸名后缀、又不精确匹配 → 未解析（实测 `xai_tool_runtime::Tool`×59）。
  纯 syn 看不穿 re-export，**这正是 Phase 2 SCIP 的领域**。未解析的 ~1.5% 主要就是
  这类 + 外部 std trait（`Default`×211、`From`×142 等，合理未解析）。
- **stray 非成员文件仍被兜到 host crate**。P1-B 修好了显式 `exclude` 的 fixture，但
  仓库根下**未被 exclude 的**非成员 `.rs`（如某些 fixture）仍经 `source_unit` 的
  `package_dir` 回退挂到 root crate 命名空间。根治需"只索引 target 模块树可达的文件、
  其余跳过"的策略，属 P1-B 的后续。
- **增量有 ~45s 地板**（见 §6 Phase 4 性能）：0 改动重跑仍要跑全图 resolve+validate。
- **CI 卫生**：Phase 1 手写代码曾漏 `cargo fmt`，挂了 Rust Checks / guard 的 Format
  步。收尾必须 `cargo fmt --check` + `cargo clippy` 一起跑，别只跑 clippy。
