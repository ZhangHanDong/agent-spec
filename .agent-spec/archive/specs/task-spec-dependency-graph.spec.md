spec: task
name: "Spec 依赖图与 DOT 可视化"
inherits: project
tags: [done, bootstrap, cli, planning, phase7]
depends: []
estimate: 1d
---

## 意图

在多个 spec 编写完成后，自动提取 spec 之间的依赖关系并生成 DOT 有向图，
让团队直观看到任务编排顺序、关键路径和工作量分布。
解决"9 个 spec 写完了但依赖关系只在人脑中"的问题。

## 已定决策

- 新增 `agent-spec graph` 命令，扫描 spec 目录生成 DOT 输出
- Spec 间依赖通过 frontmatter 新增 `depends` 字段声明
- DOT 节点用 shape 区分 spec 状态：box（待实施）、doubleoctagon（已完成）
- DOT 节点用 label 显示 spec 名称 + 预估工作量（如果有）
- DOT 边表示依赖关系
- 支持 `--format dot`（默认）和 `--format svg`（需系统安装 graphviz）
- 工作量估算来自 spec frontmatter 的 `estimate` 字段（可选）
- 关键路径用红色边标记

## 边界

### 允许修改
- src/spec_core/**
- src/spec_parser/**
- src/main.rs

### 禁止做
- 不要引入 graphviz 作为 Rust 依赖（用 std::process::Command 调用系统 dot）
- 不要修改现有 spec 的解析行为
- 不要强制要求所有 spec 声明 depends

## 完成条件

场景: 生成 DOT 依赖图
  测试:
    包: agent-spec
    过滤: test_graph_generates_dot_output
  假设 specs 目录包含 3 个 spec，其中 B depends A，C depends A 和 B
  当 执行 `agent-spec graph --spec-dir specs`
  那么 输出合法的 DOT 有向图
  并且 包含 A → B 和 A → C 和 B → C 三条边

场景: DOT 节点包含工作量估算
  测试:
    包: agent-spec
    过滤: test_graph_nodes_include_estimate
  假设 某个 spec 的 frontmatter 包含 `estimate: 2d`
  当 生成 DOT 图
  那么 该节点的 label 包含 "2d"

场景: 无依赖的 spec 作为独立节点
  测试:
    包: agent-spec
    过滤: test_graph_independent_specs_are_isolated_nodes
  假设 specs 目录有一个无 depends 的 spec
  当 生成 DOT 图
  那么 该 spec 作为独立节点出现
  并且 没有指向它的入边（除非其他 spec depends 它）

场景: 关键路径标记
  测试:
    包: agent-spec
    过滤: test_graph_critical_path_highlighted
  假设 specs 目录包含线性依赖链 A → B → C，各自 estimate 为 1d、2d、1d
  当 生成 DOT 图
  那么 A → B → C 的边被标记为关键路径（color=red）

场景: frontmatter 解析 depends 和 estimate
  测试:
    包: agent-spec
    过滤: test_parse_spec_depends_and_estimate_fields
  假设 某个 spec 的 frontmatter 包含 `depends: [task-goal-gate]` 和 `estimate: 3d`
  当 parser 解析该 spec
  那么 SpecMeta 包含 depends 和 estimate 字段
