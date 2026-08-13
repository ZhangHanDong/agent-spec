spec: task
name: "修复继承链解析入口"
inherits: project
tags: [done, bootstrap, parser, gateway, cli]
---

## 意图

让磁盘上的任务级 `.spec` 能直接继承同目录的 `project.spec`，
使 `agent-spec` 可以先用自己的项目规则约束自己，再执行具体任务。

## 已定决策

- 磁盘入口优先从当前 spec 文件所在目录解析父级规格
- 继承链修复必须同时覆盖 gateway 与 CLI 行为
- 计划阶段产出的 Task Contract 必须包含继承得到的项目级约束

## 边界

### 允许修改
- src/spec_parser/**
- src/spec_gateway/**
- src/**

### 禁止做
- 不要硬编码仓库绝对路径
- 不要让普通磁盘用例手工传入 `search_dirs`
- 不要破坏 `from_input` 这种无文件路径的入口

## 完成条件

场景: 同目录继承 project 规格
  测试: test_load_resolves_inherited_constraints_from_spec_directory
  假设 临时目录中存在 `project.spec` 和 `task.spec`
  并且 `task.spec` 声明 `inherits: project`
  当 `SpecGateway::load` 读取 `task.spec`
  那么 解析结果中包含来自 `project.spec` 的约束
  并且 Task Contract 输出中也包含这些继承约束

场景: 磁盘入口不需要手工搜索路径
  测试: resolves_parent_from_source_directory_when_no_search_dirs_are_provided
  假设 用户直接运行 `agent-spec contract task.spec`
  当 该 `task.spec` 与 `project.spec` 位于同一目录
  那么 CLI 可以完成继承解析
  并且 用户不需要再手工提供 `search_dirs`

场景: 内存入口保持原样
  测试: test_full_lifecycle
  假设 调用 `SpecGateway::from_input`
  当 输入内容本身不包含继承链
  那么 该入口继续可用
  并且 行为与修复前保持一致
