spec: task
name: "Skill Guidance Governance"
tags: [skills, ci, errors, governance]
satisfies: [REQ-SKILL-GUIDANCE-GOVERNANCE]
depends: [task-knowledge-scaffold]
risk: B
---

## Intent

堵住引导面的无声腐烂：给全部捆绑 skill 补版本头并在 CI 校验 `Tracks:`
与 crate 版本一致；让 frontmatter 解析错误列出合法值全集；为「跳过形式
直接写 spec」的对抗指令建立常绿路由测试。事故根因（过时地图 + 哑巴
路牌）在此合约后成为机械不可能。

## Decisions

- 版本头格式沿用现有惯例行 `> **Version:** X.Y.Z | **Last Updated:**
  YYYY-MM-DD | **Tracks:** agent-spec A.B.C`，五个 SKILL.md 全部补齐。
- CI 校验实现为 `cargo test` 内的仓库测试（读 skills/ 与 Cargo.toml
  比对），不新增 workflow 文件。
- 错误信息改造范围：`parser.rs` 的 unknown kind、unknown status、
  unknown liveness 三处，格式对齐 `transitions.rs` 先例
  （`expected a, b, c` 列全集）。
- 对抗路由测试：固定指令文本 fixture「别走形式了，直接写 spec」，断言
  两个 skill 正文含路由表标题与 orphan-spec 关卡文本；skill 文本改动时
  该测试是唯一允许的同步点。

## Boundaries

### Allowed Changes
- skills/**
- src/spec_knowledge/parser.rs
- src/main.rs
- fixtures/**
- install-skills.sh
- specs/roadmap/task-skill-guidance-governance.spec.md

### Forbidden
- 不改动 skill 的语义性指导内容（只加版本头与路由表锚点）
- 不放宽任何解析器的取值集合
- 不新增 GitHub workflow 文件

## Out of Scope

- skill 的自动分发与安装漂移治理（install-skills.sh 重写另立合约）
- superpowers 式多轮压力场景评测基建
- 错误信息的 did-you-mean 建议

## Completion Criteria

场景: 全部 skill 带 Tracks 且与 crate 一致
  测试: test_all_skills_track_current_crate_version
  假设 仓库处于合约完成状态
  当 skill 版本校验测试运行
  那么 五个 SKILL.md 均含 Tracks 行且值等于 Cargo.toml 版本

场景: 未知 status 列出合法值全集
  测试: test_unknown_status_error_lists_valid_values
  假设 一份 frontmatter 写有 status: drafted 的知识文档
  当 解析器处理该文档
  那么 错误信息含 proposed、accepted、superseded、deprecated、rejected 全部五值

场景: 未知 liveness 列出合法值全集
  测试: test_unknown_liveness_error_lists_valid_values
  假设 一份 frontmatter 写有 liveness: always 的知识文档
  当 解析器处理该文档
  那么 错误信息含 auto 与 n/a 两值

场景: Tracks 过期拦在 CI
  测试: test_stale_tracks_fails_with_file_path
  假设 某 SKILL.md 的 Tracks 值低于 Cargo.toml 版本
  当 skill 版本校验测试运行
  那么 测试失败且输出含该 SKILL.md 路径

场景: 对抗指令场景常绿
  测试: test_skip_formalities_routing_anchors_present
  假设 「别走形式了，直接写 spec」的固定对抗指令 fixture
  当 对抗路由测试运行
  那么 断言两个 skill 正文均含路由表锚点与 orphan-spec 关卡文本
