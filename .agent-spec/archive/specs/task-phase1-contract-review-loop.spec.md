spec: task
name: "Phase 1：Contract Review Loop"
inherits: project
tags: [done, roadmap, planned, phase1, review]
---

## 意图

把 `agent-spec` 从“验证工具”升级成“Contract 取代代码 diff 的 review 入口”，
先交付 reviewer 真正会用到的摘要和 PR 集成，而不是先做侵入式 VCS 改写。

## 已定决策

- `agent-spec explain` 是本阶段第一优先级
- `--format markdown` 与 PR description 复用同一套 explain 渲染
- `stamp` 第一版默认安全，不应默认通过 `git commit --amend` 改写历史
- GitHub Actions 示例属于交付的一部分，但保持为文档与样例，不把 GitHub 逻辑硬编码进核心验证管道

## 边界

### 允许修改
- src/**
- src/spec_report/**
- README.md
- .github/workflows/**
- specs/**

### 禁止做
- 不要先做 destructive `stamp` 再做 `explain`
- 不要让 explain 退化成单纯复制 lifecycle JSON
- 不要把 GitHub CLI 作为 explain 的必需依赖

## 完成条件

场景: explain 生成人类可读的 Contract 摘要
  测试:
    包: agent-spec
    过滤: test_explain_command_renders_contract_review_summary
  假设 某个 task spec 已通过 lifecycle
  当 用户运行 `agent-spec explain task.spec`
  那么 输出包含 Intent、Decisions、Boundaries 与 Verification Summary
  并且 适合作为 reviewer 的一屏摘要

场景: explain 生成 PR description markdown
  测试:
    包: agent-spec
    过滤: test_explain_markdown_output_is_suitable_for_pr_description
  假设 某个 task spec 需要生成 PR 说明
  当 用户运行 `agent-spec explain task.spec --format markdown`
  那么 输出为结构化 markdown
  并且 不要求用户额外拼装 Contract 摘要

场景: stamp 默认安全且支持预览
  测试:
    包: agent-spec
    过滤: test_stamp_dry_run_outputs_trailers_without_rewriting_history
  假设 某个 commit 对应的 Contract 已通过验证
  当 用户运行 `agent-spec stamp --dry-run`
  那么 输出包含将要写入的 trailer
  并且 默认不直接改写 commit 历史

## 排除范围

- run log 与 explain --history
- jj change scope
- 真实 AI backend
