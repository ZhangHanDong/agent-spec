spec: task
name: "Atlas SCIP Semantic Overlay (Phase 2)"
tags: [atlas, code-graph, scip, semantic, rust-analyzer]
satisfies: [REQ-ATLAS-SCIP-SEMANTIC]
depends: [task-rust-atlas-code-graph, task-atlas-kll-integration]
estimate: 1w
---

## Intent

把 rust-atlas 的 `Provenance::Scip` 层从"仅把 SCIP occurrence 转成 `References` 边"
升级为真正的语义 overlay:接受 rust-analyzer 直接产出的 protobuf `index.scip`,按目标
符号种类区分 `Calls`/`UsesType`,用 SCIP 精确解析 `ImplsTrait`/`ImplFor`(修复 syn
基线对 re-export/跨 crate trait 解析不了的 finding A),并让语义层在增量刷新后存活。
语法基线(`Provenance::Syn`)必须保持离线 always-on,SCIP 层是 opt-in 的可选叠加。

<!-- lint-ack: output-mode-coverage — scip-gen 的 index.scip 文件产出需真实 rust-analyzer,无法在单测断言;file-output 由 scip-gen 优雅失败场景间接覆盖 -->
<!-- lint-ack: bdd-rule-grouping — 10 个场景按 P2-1/P2-2/P2-3 三里程碑线性排列,扁平结构即可 -->


## Decisions

- 格式桥用 `scip` crate(0.9,基于 `protobuf` runtime)直接读 protobuf,不 shell out
  到外部 `scip` CLI;`overlay_scip` 按内容分派:UTF-8 且首非空字符为 `{` 走既有
  serde_json 路径,否则按 protobuf 解码,二者归一到同一内部模型。
- SCIP 边种类按**目标符号 kind** 分类:`Method`/`Function`/`TraitMethod`/`Macro` →
  `Calls`;`Struct`/`Enum`/`Trait`/`TypeAlias`/`Union`/`Field`/`EnumMember` →
  `UsesType`;其余回退 `References`。kind 取自 `SymbolInformation`(含各文档 `symbols`
  与顶层 `external_symbols`)。
- `ImplsTrait`/`ImplFor` 的精确解析不依赖 SCIP `relationships`(实测 rust-analyzer
  1.92.0 不填该字段),改用 impl 声明行上的 ref-occurrence:该行 kind=`Trait` 的目标
  →`ImplsTrait`,kind=`Struct`/`Enum`/`Union` 的目标→`ImplFor`。
- SCIP 边一律**增量新增**(`provenance=Scip`),绝不就地改写 syn 边——保持"无 `--scip`
  即 `remove_scip_edges` 干净还原"的可逆性不变量。目标在图内→`Resolved`(`to`=node id);
  目标是本仓库外符号(如 `serde::Serialize`)→`External`(`to`=SCIP 符号串,`from`
  仍为真 node,通过 `validate_graph`)。
- `Capability` 复活 `scip_index: Option<String>` + 新增 `scip_fingerprint:
  Option<String>`(blake3 of index 文件),经 `#[serde(default)]` 向后兼容;
  `SCHEMA_VERSION` 递增。
- `refresh` 读上次 `Capability`:若记录了仍存在的 SCIP index,增量 build 自动 re-overlay
  而非用 `BuildOptions::default()` 清空。index 文件消失→退回纯 syn + 清 Scip 边。
- 新增 `atlas scip-gen`:调用 `rust-analyzer scip <code>` 产 `index.scip` 落到
  graph_dir;二进制缺失/项目不可编译→stderr 明确 warning + 非零退出,`build` 侧
  `--scip` 缺失时始终退回纯 syn,绝不 panic。

## Boundaries

### Allowed Changes
- crates/rust-atlas/**
- src/main.rs
- fixtures/atlas/**
- specs/task-atlas-scip-semantic.spec.md
- docs/atlas-roadmap.md

### Forbidden
- 不改 `Provenance` / `EdgeKind` / `EdgeResolution` 枚举的既有变体语义
- 不让 SCIP 成为 build 的硬依赖:无 index 时纯 syn 路径行为不变
- 不引入 `unwrap`/`expect`/`unsafe`(crate lint 已 deny)
- 不就地改写或删除 `Provenance::Syn` 边

## Out of Scope

- Phase 3 tree-sitter 多语言基线
- Phase 4 rustc MIR 调用图 / 数据流
- rust-analyzer 版本/toolchain 自动探测与安装
- 把 SCIP 生成接入 CI(实测数字手工记录到 roadmap 即可)

## Completion Criteria

场景: 读取 protobuf index.scip 产出 resolved reference 边
  测试: test_overlay_reads_protobuf_index
  假设 一个 crate 含 `impl Store for MemStore` 且已由 rust-analyzer 产出 protobuf `index.scip`
  当 以该 protobuf index 运行 `atlas build --scip index.scip`
  那么 `capability.scip` 为 true
  并且 图中存在至少一条 `provenance=scip` 且 `resolution=resolved` 的边

场景: 既有 JSON index 路径保持可用
  测试: test_overlay_still_reads_json_index
  Level: integration
  假设 一个 JSON 形态的 SCIP index fixture
  当 以该 JSON index 运行 build
  那么 `capability.scip` 为 true
  并且 产出的 scip 边与升级前语义一致

场景: 方法调用产出 Calls 边
  测试: test_scip_emits_calls_for_method_target
  假设 index 中某引用的目标符号 kind 为 `Method` 或 `Function`
  当 overlay 消费该引用
  那么 生成的边 kind 为 `calls`

场景: 类型引用产出 UsesType 边
  测试: test_scip_emits_usestype_for_type_target
  假设 index 中某引用的目标符号 kind 为 `Struct` 或 `Trait`
  当 overlay 消费该引用
  那么 生成的边 kind 为 `uses-type`

场景: SCIP 精确解析 ImplsTrait 到 resolved node id
  测试: test_scip_resolves_impls_trait_edge
  假设 `impl Store for MemStore` 且 `Store` 在图中有定义节点
  当 SCIP overlay 处理 impl 声明行
  那么 存在一条 `provenance=scip` 的 `impls-trait` 边指向 `Store` 的 node id
  并且 该边 `resolution=resolved`

场景: 跨 crate 外部 trait 标为 External 而非 Resolved
  测试: test_scip_external_trait_marked_external
  假设 `impl` 的目标 trait 定义在本仓库外(图中无该 node)
  当 SCIP overlay 处理该 impl
  那么 对应边 `resolution=external`
  并且 该边不违反 `validate_graph`(resolved 端点校验被跳过)

场景: 缺 rust-analyzer 时 scip-gen 优雅失败
  测试: test_scip_gen_missing_binary_warns
  假设 指定的 rust-analyzer 二进制不存在
  当 运行 `atlas scip-gen --ra /nonexistent/ra`
  那么 命令返回非零错误并在 stderr 说明缺少 rust-analyzer
  但是 不发生 panic

场景: SCIP overlay 跨增量刷新存活
  测试: test_scip_survives_incremental_refresh
  Level: integration
  假设 已用 `--scip` build 且 `Capability` 记录了仍存在的 index 路径与指纹
  当 一个源文件改动后触发增量 refresh
  那么 刷新后图中仍存在 `provenance=scip` 边
  并且 `capability.scip` 仍为 true

场景: 记录的 SCIP index 消失后退回纯 syn
  测试: test_scip_missing_index_falls_back_to_syn
  假设 `Capability` 记录的 SCIP index 文件已被删除
  当 触发增量 refresh
  那么 图中不再存在 `provenance=scip` 边
  并且 `capability.scip` 为 false

场景: 无 SCIP 时纯 syn 路径行为不变
  测试: test_build_without_scip_stays_syn_only
  Level: integration
  假设 不传 `--scip` 且无记录的 index
  当 运行 build
  那么 所有边 `provenance=syn`
  并且 `capability.scip` 为 false
