spec: task
name: "正式化场景到测试的绑定"
inherits: project
tags: [done, bootstrap, verify, parser, contract]
---

## 意图

把任务合约里的完成条件与 Rust 测试之间的绑定，
从临时的 `// @spec:` 注释约定升级为 spec 内可声明的正式机制。

## 已定决策

- 场景可用 `测试:` / `Test:` 显式声明 Rust test selector
- `TestVerifier` 优先使用场景内显式 selector
- 旧的 `// @spec:` 注释继续保留为兼容 fallback

## 边界

### 允许修改
- src/spec_core/**
- src/spec_parser/**
- src/spec_verify/**
- src/**
- specs/**

### 禁止做
- 不要移除现有 `// @spec:` 兼容能力
- 不要要求所有旧 spec 一次性迁移
- 不要把测试绑定继续留在只靠源码注释的状态

## 完成条件

场景: 场景可显式声明测试选择器
  测试: test_parse_scenario_with_explicit_test_selector
  假设 某个场景块包含 `测试:` 行
  当 parser 解析该场景
  那么 AST 中保留 `test_selector`
  并且 JSON 输出中也包含该 selector

场景: 显式测试选择器优先于旧注释映射
  测试: test_explicit_scenario_selector_takes_precedence_over_legacy_comment_binding
  假设 同一个场景同时存在显式 selector 和旧注释映射
  当 TestVerifier 解析绑定关系
  那么 显式 selector 优先
  并且 不再依赖场景名去匹配测试函数

场景: 旧版注释绑定继续兼容
  测试: test_legacy_comment_binding_is_used_when_no_explicit_selector_exists
  假设 某个场景没有显式 selector
  当 TestVerifier 解析绑定关系
  那么 旧版 `// @spec:` 映射仍然可用
  并且 现有自举规格不需要一次性全部迁移
