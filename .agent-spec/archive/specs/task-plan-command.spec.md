spec: task
name: "实现计划生成（Plan Command）"
inherits: project
tags: [done, bootstrap, lifecycle, phase9]
depends: [task-spec-dependency-graph]
estimate: 1.5d
---

## 意图

在 Spec → Implement 之间补全缺失的 Plan 层。
`agent-spec plan` 读取 spec + 扫描代码目录，输出结构化的实现计划上下文，
让 AI Agent 拿到足够信息后能直接生成高质量的 implementation plan，
而不需要自己猜测代码结构和现有模式。

这是 Spec Driven Development 四步流程（Spec → Plan → Tasks → Implement）中
agent-spec 当前唯一缺失的一环。Plan 的生成逻辑留给调用方 AI，
agent-spec 只负责收集和结构化 plan 所需的上下文。

## 已定决策

- 新增 CLI 子命令 `agent-spec plan <spec> --code <dir>`
- 输出由三个区块组成：Contract（已有）、Codebase Context（新增）、Task Sketch（新增）
- Codebase Context 通过扫描 `--code` 目录收集：
  - Boundaries 中 Allowed Changes 路径下的现有文件列表（递归，去 gitignore）
  - 每个文件的首行摘要（模块 doc comment 或 pub struct/fn 签名）
  - 现有测试文件及其 `#[test]` / `#[tokio::test]` 函数名列表
- Task Sketch 基于 spec 的场景自动分组：
  - 按场景的 `前置` 依赖做拓扑排序
  - 无依赖的场景分为一组，有依赖的按层级分组
  - 每组附上涉及的 boundary 路径和关联的 test selector
- 支持 `--format text|json|prompt` 三种输出格式
  - `text`：人类可读的结构化摘要
  - `json`：机器可解析的完整上下文
  - `prompt`：专为 AI Agent 设计的 self-contained prompt（包含所有继承约束）
- `--depth shallow|full` 控制 codebase 扫描深度
  - `shallow`（默认）：只列文件名 + 首行摘要
  - `full`：包含每个文件的 pub API 签名列表
- 不执行任何 AI 推理——plan 命令是纯机械的上下文收集器

## 约束

### 必须做
- 必须复用现有 `TaskContract::from_resolved()` 生成 Contract 区块
- 必须尊重 `.gitignore` 规则（不扫描被忽略的文件）
- 必须在 Allowed Changes 路径不存在时输出警告而非报错
- `--format prompt` 输出必须是 self-contained 的（不依赖 agent-spec CLI 即可被 AI 消费）

### 禁止做
- 不要读取文件完整内容（只读首行摘要或 pub 签名）
- 不要调用任何外部 AI 服务
- 不要修改 spec 文件
- 不要引入新的外部依赖（用 std::fs 递归遍历即可）

## 边界

### 允许修改
- src/main.rs
- src/spec_gateway/**
- src/spec_report/**
- src/spec_core/**

### 禁止做
- 不要修改 src/spec_parser/**（parser 层不需要改动）
- 不要修改 src/spec_verify/**（验证层不需要改动）

## 排除范围

- AI 驱动的 plan 生成（留给调用方 Agent）
- 自动代码修改建议
- 与 IDE 的集成
- 并行安全检查（属于 graph 命令的扩展）

## 完成条件

场景: plan 输出包含 Contract 区块（critical）
  标签: critical
  测试:
    包: agent-spec
    过滤: test_plan_includes_contract_section
  假设 一个有效的 task spec 文件
  当 执行 `agent-spec plan <spec> --code .`
  那么 输出包含 Intent、Decisions、Boundaries、Completion Criteria
  并且 内容与 `agent-spec contract` 的输出一致

场景: plan 输出包含 Codebase Context 区块
  测试:
    包: agent-spec
    过滤: test_plan_includes_codebase_context
  假设 spec 的 Allowed Changes 包含 `src/spec_gateway/**`
  当 执行 `agent-spec plan <spec> --code <project-root>`
  那么 输出包含该路径下的文件列表
  并且 每个文件附有首行摘要

场景: plan 输出包含 Task Sketch 区块
  测试:
    包: agent-spec
    过滤: test_plan_includes_task_sketch
  假设 spec 有 4 个场景，其中场景 C 依赖场景 A
  当 执行 `agent-spec plan <spec> --code .`
  那么 Task Sketch 将场景分为至少 2 组
  并且 场景 A 所在组排在场景 C 所在组之前

场景: plan 扫描尊重 gitignore
  测试:
    包: agent-spec
    过滤: test_plan_respects_gitignore
  假设 项目有 `.gitignore` 排除 `target/` 目录
  当 执行 `agent-spec plan <spec> --code .`
  那么 Codebase Context 中不包含 `target/` 下的文件

场景: plan --format json 输出可解析的 JSON
  测试:
    包: agent-spec
    过滤: test_plan_json_format_is_valid
  假设 一个有效的 task spec 文件
  当 执行 `agent-spec plan <spec> --code . --format json`
  那么 输出是合法的 JSON
  并且 包含 `contract`、`codebase_context`、`task_sketch` 三个顶层字段

场景: plan --format prompt 输出 self-contained prompt
  测试:
    包: agent-spec
    过滤: test_plan_prompt_format_is_self_contained
  假设 一个继承了 project spec 的 task spec
  当 执行 `agent-spec plan <spec> --code . --format prompt`
  那么 输出包含从 project spec 继承的约束
  并且 输出不包含 "run agent-spec" 等 CLI 依赖指令

场景: plan --depth full 输出 pub API 签名
  测试:
    包: agent-spec
    过滤: test_plan_full_depth_includes_pub_signatures
  假设 Allowed Changes 路径下有 Rust 源文件
  当 执行 `agent-spec plan <spec> --code . --depth full`
  那么 Codebase Context 包含该文件的 `pub fn`/`pub struct`/`pub enum` 签名

场景: Allowed Changes 路径不存在时输出警告
  测试:
    包: agent-spec
    过滤: test_plan_warns_on_missing_boundary_path
  假设 spec 的 Allowed Changes 包含一个不存在的路径 `src/nonexistent/**`
  当 执行 `agent-spec plan <spec> --code .`
  那么 命令不报错（退出码 0）
  并且 输出包含该路径的警告信息

场景: plan 输出测试文件中的 test 函数名列表
  测试:
    包: agent-spec
    过滤: test_plan_lists_existing_test_functions
  假设 Allowed Changes 路径下有包含 `#[test]` 的文件
  当 执行 `agent-spec plan <spec> --code .`
  那么 Codebase Context 包含该文件的 test 函数名列表
