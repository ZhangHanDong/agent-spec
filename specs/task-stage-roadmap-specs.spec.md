spec: task
name: "分阶段落盘后续改进路线图"
inherits: project
tags: [bootstrap, roadmap, planning]
---

## 意图

把新的改进计划重写成一组可自举的 task specs，
让后续阶段不再只存在于长文计划里，而是作为仓库内可被 `agent-spec contract` 消费的路线图合同存在。

## 已定决策

- 未来阶段的 roadmap task specs 存放在 `specs/roadmap/`
- roadmap specs 在被提升到顶层 `specs/` 前，不进入默认 top-level guard
- roadmap 按 concern 分阶段拆分：Phase 0 Contract fidelity、Phase 1 review loop、Phase 2 traceability、Phase 3 governance、Phase 4 AI、Phase 5 ecosystem、Phase 6 advanced verification
- 已验证完成的 roadmap specs 通过 `agent-spec archive` 移入 `.agent-spec/archive/specs/`（带 lifecycle 证据），阶段合同内容在归档位置继续可查
- 嵌套 roadmap specs 仍应继承顶层 `specs/project.spec`

## 边界

### 允许修改
- src/spec_parser/**
- src/**
- specs/**

### 禁止做
- 不要把未来 backlog spec 直接堆到顶层 `specs/` 并让默认 guard 变红
- 不要把多个阶段重新压回一个大而全的“万能 spec”
- 不要在路线图拆分时改动当前验证语义

## 完成条件

场景: 嵌套 roadmap spec 继续继承顶层 project 规则
  测试:
    过滤: resolves_parent_from_nested_spec_directory_via_ancestor_specs_dir
  假设 某个 roadmap task spec 位于 `specs/roadmap/`
  当 parser 默认解析其继承链
  那么 顶层 `specs/project.spec` 仍可被发现
  并且 用户不需要为 roadmap 目录单独配置搜索路径

场景: Phase 0 与 Phase 1 roadmap spec 已拆分并表达正确优先级
  测试:
    包: agent-spec
    过滤: test_roadmap_phase_zero_and_one_specs_exist_and_capture_priorities
  假设 仓库内存在已归档的 phase task specs（`.agent-spec/archive/specs/`）
  当 用户查看前两个 phase spec
  那么 Phase 0 优先修 Contract fidelity
  并且 Phase 1 聚焦 explain、markdown PR 摘要与安全版 stamp

场景: 后续 roadmap spec 按 concern 分层
  测试:
    包: agent-spec
    过滤: test_roadmap_later_phase_specs_exist_and_are_split_by_concern
  假设 用户继续查看已归档的后续 phase spec
  当 用户对比 Phase 2 到 Phase 6
  那么 过程追踪、Spec 治理、AI 扩展、生态集成与高级验证被拆成独立任务
  并且 `phase:` front matter 没有被纳入主路线

场景: roadmap README 说明 staging 与 promotion 规则
  测试:
    包: agent-spec
    过滤: test_roadmap_readme_documents_promotion_rule
  假设 用户查看 `specs/roadmap/README.md`
  当 用户阅读 roadmap 目录说明
  那么 说明文档指出 roadmap specs 暂不进入默认 guard
  并且 说明它们在实现启动时会被提升到顶层 `specs/`
