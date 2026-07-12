spec: task
name: "支持场景验证强度元数据"
inherits: project
tags: [done, dsl, parser, lint, verification, phase-next]
---

## 意图

为 `agent-spec` 增加表达测试强度所需的可选场景元数据。
当前 `测试:` 只能绑定名称，无法区分 unit、integration、CLI、e2e，也无法表达场景是否依赖 local stub、fixture filesystem 或真实协议边界。
本任务为高风险 I/O 场景引入可选的验证元数据。

## 已定决策

- 在现有 `测试:` selector 基础上新增可选元数据行：`层级:`、`替身:`、`命中:`
- 英文关键字同时支持 `Level:`、`Test Double:`、`Targets:`
- 这些字段全部为可选；旧 spec 和旧单行 `测试:` 写法保持兼容
- parser、AST、JSON 输出和 `contract` 渲染都必须保留这些元数据
- lint 只在高风险外部 I/O 场景上鼓励这些字段，不要求所有场景都填写

## 边界

### 允许修改
- src/spec_parser/**
- src/spec_core/**
- src/spec_lint/**
- src/spec_gateway/**
- src/**
- specs/**
- README.md
- docs/**

### 禁止做
- 不要破坏现有 `测试:` 单行或结构化块写法
- 不要把元数据做成 task spec 的强制字段
- 不要只更新 parser 而遗漏 JSON 输出或 contract 渲染

## 完成条件

场景: parser 解析中文验证强度元数据并保留到 AST
  测试:
    过滤: test_parse_scenario_verification_metadata_fields
  假设 某个场景在 `测试:` 后声明 `层级:`、`替身:` 与 `命中:`
  当 parser 读取该 spec
  那么 AST 中保留这些字段
  并且 旧的 `测试:` selector 仍正常解析

场景: parser 同时支持英文验证强度元数据关键字
  测试:
    过滤: test_parse_english_verification_metadata_fields
  假设 某个英文 spec 在 `Test:` 后声明 `Level:`、`Test Double:` 与 `Targets:`
  当 parser 读取该 spec
  那么 AST 中保留这些字段
  并且 英文关键字与中文关键字具有等价语义

场景: JSON 输出与 contract 渲染保留元数据
  测试:
    过滤: test_contract_and_json_output_preserve_verification_metadata
  假设 某个场景包含验证元数据
  当 运行 `agent-spec parse --json` 和 `agent-spec contract`
  那么 JSON 输出与文本 contract 都保留这些字段
  并且 不会丢失原有测试选择器内容

场景: 旧规格文件保持兼容
  测试:
    过滤: test_existing_specs_without_verification_metadata_remain_valid
  假设 现有仓库中的 task spec 都没有使用 `层级:`、`替身:` 或 `命中:`
  当 运行 parser 与 lint
  那么 这些旧 spec 继续通过
  并且 不需要任何迁移即可使用新版本工具

场景: 高风险 I/O 场景缺少元数据时得到建议
  测试:
    过滤: test_lint_suggests_verification_metadata_for_external_io_scenarios
  假设 某个场景描述 HTTP、filesystem 或 protocol 错误路径
  当 该场景没有填写 `层级:`、`替身:` 或 `命中:`
  那么 lint 给出建议性 warning
  并且 warning 解释这些字段有助于表达测试强度

## 排除范围

- 根据元数据自动执行不同测试框架
- 把验证元数据直接接入 AI verifier 推理
- 为所有场景强制要求填写元数据
