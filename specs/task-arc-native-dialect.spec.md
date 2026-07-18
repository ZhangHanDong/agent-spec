spec: task
name: "ARC-Native Requirements Dialect"
tags: [intent-compiler, yaml, arc, interop]
satisfies: [REQ-ARC-NATIVE-DIALECT]
depends: [task-intent-compiler-yaml-frontend, task-intent-compiler-yaml-export]
estimate: 2d
---

## Intent

让 agent-spec 编译出的需求可以直接作为参照编译器（ARC）的输入
`requirements.yaml`，并让 ARC 原生需求树可以进入意图编译器接受治理。读侧：
`requirements import` 自动识别 ARC 原生形状并映射进 IR（含块标量与空 flow
列表的解析器扩展）；写侧：`requirements export --dialect arc-native` 把 IR
投影为参照装载器可直接消费的单根树；两侧以往返不动点与真实参照文件 fixture
互证。

## Decisions

- 识别规则：顶层映射带 `id:` 且无 `requirements:` 键，或 `root:`/`requirement:` 包装 → ARC 原生路径；否则维持 v1.1 行为不变。
- 解析器扩展仅限 ARC 原生路径需要的构造：`>-`/`>`/`|`/`|-` 块标量（折叠语义：行以空格连接，更深缩进保留换行）与空 flow 列表 `[]`；锚点与非空 flow 集合保持 `yaml-unsupported-construct` 指名拒绝。
- 字段映射：`name`↔`title`；FOLDER `description`↔`## Problem`；ATOMIC `description`↔条款语句；导入跳过根节点，导出合成根（`id: ROOT`，`--root-name` 定名，默认 `Requirements`）。
- 点号 id 规范化为连字符入 IR（`REQ-1.1`→`REQ-1-1`），仅在发生改写时于 frontmatter 写 `source-id:` 保真行；arc-native 导出优先用 `source-id` 还原。
- 场景双向走 `steps: [{keyword, content}]`，导出关键字大写（GIVEN/WHEN/THEN/AND/BUT）；ATOMIC 场景并入所属文档 `## Scenarios`；文档级场景导出挂在对应 FOLDER 节点（放置规则如实写入文档）。
- 导出产物为普通两空格缩进 YAML（无需块标量），满足参照装载器 `yaml.safe_load` + 根 id 非空即可加载；`--check` 漂移门语义与 v1.1 导出一致。
- 真实参照文件的逐字节副本放 `fixtures/arc-native/requirements.yaml`；既有 parity 合同的输入一致性测试升级为绑定该真实文件。

## Boundaries

### Allowed Changes
- src/spec_knowledge/**
- src/main.rs
- fixtures/arc-native/**
- fixtures/requirements-parity/**
- specs/task-arc-native-dialect.spec.md
- specs/task-reference-compiler-parity.spec.md
- knowledge/requirements/req-arc-native-dialect.md
- docs/intent-compiler/**
- book/src/ch10-intake.md
- skills/agent-spec-tool-first/**
- CHANGELOG.md

### Forbidden
- 不改变 v1.1 方言对既有输入的行为。
- 不添加 YAML 库依赖（手写子集解析器按需扩展）。
- 不虚构参照装载器语义（以其源码 load_requirements 为准）。

## Out of Scope

- ARC traceability.db 的读写（运行时产物，非输入格式）
- ARC reference/ 视觉资产的搬运（导出仅产 requirements.yaml）
- 非空 flow 集合与锚点的支持

## Questions

- [x] ATOMIC 级场景在 IR 无条款级场景槽位时如何保真？（已解决：并入文档级 `## Scenarios`——与 v1.1 leaf 场景既有语义一致；导出放置规则如实文档化，需要逐 ATOMIC 场景的用户写单条款文档。）
- [x] `[]` 支持是否改变 v1.1 行为？（已解决：解析器为双方言共享，空 flow 列表从"拒绝"放宽为"解析为空列表"——此前该输入是硬错误，无既有用户依赖被破坏；非空 flow 集合仍拒绝。Forbidden 条款据此理解为"既有可解析输入的行为不变"。）

## Completion Criteria

<!-- lint-ack: bdd-rule-grouping — 交换方言合同，场景按读/写/负路径清单守卫 -->

Scenario: 真实参照文件逐字节导入成功
  Test:
    Filter: test_arc_native_real_ticketbooking_imports_cleanly
    Level: integration
  Given fixtures/arc-native/requirements.yaml（参照项目示例的逐字节副本）
  When requirements import 运行
  Then 导入成功且零 yaml-unsupported-construct 诊断
  And 生成文档的 title 来自 name 字段且条款语句来自 ATOMIC description

Scenario: 块标量与空 flow 列表被正确解析
  Test:
    Filter: test_arc_native_block_scalars_and_empty_flow_parse
    Level: integration
  Given 含 `>-` 折叠标量与 `dependencies: []` 的 ARC 原生树
  When ARC 原生读取运行
  Then 折叠文本按行以空格连接
  And 空 flow 列表映射为空依赖

Scenario: 点号 id 规范化且可还原
  Test:
    Filter: test_arc_native_dotted_ids_normalize_with_source_id
    Level: integration
  Given 节点 id 为 `REQ-1.1` 的 ARC 原生树
  When 导入后再以 arc-native 方言导出
  Then IR 文档 id 为 `REQ-1-1` 且 frontmatter 含 `source-id: REQ-1.1`
  And 导出树中的节点 id 还原为 `REQ-1.1`

Scenario: 导出产物可被参照装载器消费
  Test:
    Filter: test_arc_native_export_is_reference_loadable
    Level: integration
  Given agent-spec 语料
  When requirements export 以 --dialect arc-native 运行
  Then 产物是单根映射、根 id 非空、FOLDER/ATOMIC 与 steps 场景形状齐备

Scenario: 往返不动点
  Test:
    Filter: test_arc_native_round_trip_fixpoint
    Level: integration
  Given agent-spec 语料的 arc-native 导出
  When 导入该导出并再次导出
  Then 两次导出逐字节相同

Scenario: 锚点被指名拒绝
  Test:
    Filter: test_arc_native_rejects_anchor_constructs
    Level: integration
  Given 含 YAML 锚点的 ARC 原生树
  When ARC 原生读取运行
  Then 诊断为 yaml-unsupported-construct 且含行号

Scenario: 非空 flow 集合被指名拒绝
  Test:
    Filter: test_arc_native_rejects_nonempty_flow_collections
    Level: integration
  Given 含 `dependencies: [REQ-1]` 的 ARC 原生树
  When ARC 原生读取运行
  Then 诊断为 yaml-unsupported-construct 且含行号
