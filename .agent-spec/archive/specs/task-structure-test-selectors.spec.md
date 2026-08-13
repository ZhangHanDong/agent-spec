spec: task
name: "结构化测试选择器"
inherits: project
tags: [done, bootstrap, verify, parser, contract, phase4]
---

## 意图

把任务合约里的测试绑定从单个字符串过滤器升级成结构化模型，
让场景可以稳定声明 `package + filter`，减少同名测试和跨 crate 误匹配。

## 已定决策

- 继续支持旧的单行 `测试: test_name` / `Test: test_name` 写法
- 新增结构化 `测试:` / `Test:` 块，首批支持 `包|Package` 与 `过滤|Filter`
- `TestVerifier` 对结构化选择器按 `cargo test -p <package> <filter>` 执行

## 边界

### 允许修改
- src/spec_core/**
- src/spec_parser/**
- src/spec_verify/**
- src/**
- src/spec_gateway/**
- specs/**

### 禁止做
- 不要移除旧的字符串 selector 兼容能力
- 不要把 `测试:` 结构化块降级成原始文本拼接
- 不要要求作者直接写完整命令行而不是声明结构化字段

## 完成条件

场景: parser 保留结构化测试选择器
  测试:
    过滤: test_parse_structured_test_selector_block
  假设 某个场景使用 `测试:` 块声明 `包: spec-parser` 和 `过滤: test_parse_structured_test_selector_block`
  当 parser 解析该场景
  那么 AST 中保留结构化 `test_selector`
  并且 JSON 输出中也包含 `package` 与 `filter`

场景: 单行测试选择器继续兼容
  测试:
    过滤: test_parse_shorthand_test_selector_as_filter_only
  假设 某个场景继续使用单行 `测试: test_name`
  当 parser 解析该场景
  那么 `test_selector.filter` 等于 `test_name`
  并且 `test_selector.package` 保持为空

场景: verifier 使用 package 范围执行测试
  测试:
    过滤: test_build_cargo_test_command_with_package_selector
  假设 某个场景声明 `包: spec-parser` 和 `过滤: test_parse_structured_test_selector_block`
  当 TestVerifier 构造测试命令
  那么 命令中包含 `-p spec-parser`
  并且 继续使用过滤器 `test_parse_structured_test_selector_block`
