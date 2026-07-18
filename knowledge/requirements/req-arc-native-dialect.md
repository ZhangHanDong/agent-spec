---
kind: requirement
id: REQ-ARC-NATIVE-DIALECT
title: "ARC-Native Requirements Dialect"
status: accepted
liveness: auto
tags: [intent-compiler, yaml, arc, dialect, interop]
---

# ARC-Native Requirements Dialect

## Problem

agent-spec 编译出的需求要能直接作为参照编译器（ARC）的输入 `requirements.yaml`
使用；反向地，ARC 原生的需求树也要能进入意图编译器接受治理。实测表明参照项目的
真实文件形状与 v1.1 交换方言差异巨大：单根节点（非 `requirements:` 列表）、
`name:` 而非 `title:`、折叠块标量、flow 空列表、`steps: [{keyword, content}]`
场景、ATOMIC 携带 `description:`、点号层级 id。需要一个 ARC 原生方言：读侧
自动识别并映射进 IR，写侧把 IR 投影为参照装载器可直接消费的树。

## Requirements

[REQ-ARC-NATIVE-DIALECT-DETECT] `requirements import` MUST 自动识别 ARC 原生形状（顶层映射带 `id:` 且无 `requirements:` 键，或 `root:`/`requirement:` 包装），并走 ARC 原生映射路径；v1.1 方言行为不变。

[REQ-ARC-NATIVE-DIALECT-BLOCK-SCALARS] ARC 原生读取 MUST 支持折叠与字面块标量（`>-`、`>`、`|`、`|-`）以及空 flow 列表 `[]`；其余 flow 集合与锚点仍以 `yaml-unsupported-construct` 指名拒绝。

[REQ-ARC-NATIVE-DIALECT-FIELD-MAP] 字段映射 MUST 为：节点 `name` ↔ IR `title`；FOLDER `description` ↔ `## Problem`；ATOMIC `description` ↔ 条款语句；根节点在导入时跳过、导出时合成。

[REQ-ARC-NATIVE-DIALECT-DOTTED-IDS] 点号层级 id（如 `REQ-1.1`）MUST 规范化为连字符形式入 IR，并在生成文档 frontmatter 写入 `source-id:` 保真行；ARC 原生导出 MUST 用 `source-id` 还原原始 id。

[REQ-ARC-NATIVE-DIALECT-SCENARIOS] 场景 MUST 以 `steps: [{keyword, content}]` 形状双向映射（关键字 GIVEN/WHEN/THEN/AND/BUT 大写导出），ATOMIC 级场景并入所属文档的 `## Scenarios`。

[REQ-ARC-NATIVE-DIALECT-EXPORT] `requirements export --dialect arc-native --out <file>.yaml` MUST 把确认后的 IR 投影为单根 ARC 树：根 `id: ROOT`、`--root-name` 可定名、FOLDER/ATOMIC 层级、依赖与场景齐备，产物可被参照装载器（`yaml.safe_load` + 根 id 非空）直接消费。

[REQ-ARC-NATIVE-DIALECT-FIXPOINT] agent-spec 语料的 ARC 原生导出 MUST 满足往返不动点：导出 → 导入 → 再导出逐字节相同。

[REQ-ARC-NATIVE-DIALECT-REAL-FIXTURE] 参照项目 ticketbooking 示例的逐字节副本 MUST 作为 fixture 导入成功且零 `yaml-unsupported-construct` 诊断。

[REQ-ARC-NATIVE-DIALECT-NEGATIVE] 满足本需求的合同 MUST 覆盖负路径：锚点与非空 flow 集合被指名拒绝。

## Scenarios

Scenario: 真实参照文件导入成功
  Given 参照项目 ticketbooking requirements.yaml 的逐字节副本
  When requirements import 运行
  Then 生成的 IR 文档带正确的 title 映射与 source-id 保真行

Scenario: 导出可被参照装载器消费
  Given agent-spec 语料
  When 以 arc-native 方言导出
  Then 产物是单根映射且根 id 非空

Scenario: 不支持构造被指名拒绝
  Given 含锚点的 ARC 风格文件
  When requirements import 运行
  Then 诊断指名 yaml-unsupported-construct 与行号

## Dependencies

- REQ-INTENT-COMPILER-YAML-FRONTEND
- REQ-INTENT-COMPILER-YAML-EXPORT

## Source Trace

- decision origin: 用户指令 2026-07-19 —— "agent-spec 编译的需求可以兼容 arc 的输入 requirements.yaml"
- 实测证据: 真实 ticketbooking 文件被 v1.1 方言在第 5 行拒绝（yaml-unsupported-construct: content outside the root block）
- 参照装载器语义: load_requirements（yaml.safe_load + 根 id 非空 + root/requirement 包装容忍）
- staged contract: specs/task-arc-native-dialect.spec.md
