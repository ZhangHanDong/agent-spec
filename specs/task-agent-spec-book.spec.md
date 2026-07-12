spec: task
name: "agent-spec 1.0 Book (Chinese Edition)"
tags: [book, docs, mdbook, dogfood]
satisfies: [REQ-AGENT-SPEC-BOOK]
estimate: 2d
---

## Intent

以 spec 驱动的方式写出 agent-spec 1.0 官方读物（中文版）：由浅入深、以用法为
主线（第一至四部分），以理念与架构收束（第五部分）。mdbook 组织，Mermaid 随文
渲染，结构质量由绑定测试机械守卫 —— 本书自己就是 agent-spec 工作流的展品。

## Decisions

- 书源在 `book/`：`book.toml` 启用 `mdbook-mermaid` 预处理器；`book/src/SUMMARY.md` 为唯一目录真相。
- 章节结构按 `docs/book-outline.md`：前言 + 19 章（五部分）+ 附录 A-D；中文先行，英文版在前言中显式声明为后续工作。
- 每章开篇 `> **定位**` 锚（含 baseline `agent-spec 1.0.0` 标注），每章至少一个 ```mermaid 块。
- 前言含阅读路径与知识地图：路径至少两条，知识地图为 Mermaid。
- 附录 C 收录本书需求与合同（自举）；附录 D 含两条跨三章以上的 E2E 轨迹。
- 结构守卫测试放在 `src/main.rs` tests：仅做文件与文本形状检查，不依赖 mdbook 二进制（CI 无需安装 mdbook）。
- 用法章节的命令输出示例来自真实运行（1.0.0 CLI），不得虚构输出。

## Boundaries

### Allowed Changes
- book/**
- docs/book-outline.md
- src/main.rs
- knowledge/requirements/req-agent-spec-book.md
- specs/task-agent-spec-book.spec.md
- .github/workflows/static.yml
- docs/index.html
- CHANGELOG.md
- .gitignore

### Forbidden
- 不改动任何编译器行为代码（本任务只新增测试与书稿）。
- 不虚构命令输出或版本号。
- 不把英文版缺失当作中文版交付的阻塞。

## Out of Scope

- 英文全书翻译（后续任务）
- 书籍 PDF/EPUB 产出
- 交互式可视化（Mermaid 静态图即全部）

## Questions

- [x] mdbook 构建是否进 CI？（已解决：结构测试进 cargo test 守 CI；mdbook 构建放 Pages 部署工作流，用预编译二进制安装，与 harper 同模式。）

## Completion Criteria

<!-- lint-ack: bdd-rule-grouping — 书稿合同，场景以清单式守卫为主，暂不分 Rule -->
<!-- lint-ack: output-mode-coverage — 本任务的"输出"即书稿文件本身，其存在性、形状与内容正是七个场景逐项断言的对象 -->
<!-- lint-ack: decision-coverage — "命令输出来自真实运行"是写作纪律，机械测试无法证伪"未虚构"；由 tech-writer 评审阶段的 fact-checker agent 承担 -->
<!-- lint-ack: observable-decision-coverage — 同上：真实性由评审 agent 核对，结构测试守形状 -->

Scenario: SUMMARY 完整且章节文件齐备
  Test:
    Filter: test_book_summary_lists_chapters_and_files_exist
    Level: integration
  Given `book/src/SUMMARY.md`
  When 结构守卫测试解析目录中的每个链接
  Then 每个链接的文件存在且长度大于 500 字节
  And 目录覆盖前言、19 章与附录 A-D

Scenario: 章章有定位锚与 Mermaid 图
  Test:
    Filter: test_book_chapters_carry_anchor_baseline_and_mermaid
    Level: integration
  Given 全部章节文件（前言与附录 A/B 除外的每一章）
  When 逐章扫描文本
  Then 每章含 `> **定位**` 开篇锚与 baseline 标注
  And 每章至少含一个 mermaid 代码块

Scenario: mdbook-mermaid 已配置
  Test:
    Filter: test_book_toml_configures_mermaid_preprocessor
    Level: integration
  Given `book/book.toml`
  When 读取配置
  Then 存在 `[preprocessor.mermaid]` 且附加了 mermaid 资源脚本

Scenario: 前言含阅读路径与知识地图
  Test:
    Filter: test_book_preface_has_reading_paths_and_knowledge_map
    Level: integration
  Given 前言文件
  When 扫描文本
  Then 至少两条阅读路径（路径 A/路径 B）与一个 mermaid 知识地图存在

Scenario: 自举附录收录本书契约
  Test:
    Filter: test_book_dogfood_appendix_embeds_own_contract
    Level: integration
  Given 附录 C
  When 扫描文本
  Then 内容包含 REQ-AGENT-SPEC-BOOK 与本合同文件名

Scenario: E2E 轨迹跨章且有图
  Test:
    Filter: test_book_traces_span_chapters_with_diagrams
    Level: integration
  Given 附录 D
  When 扫描文本
  Then 至少两条轨迹，各引用三个以上章节且含 mermaid 图

Scenario: 缺失章节被机械拒绝
  Test:
    Filter: test_book_guard_fails_on_missing_chapter_fixture
    Level: integration
  Given 一份引用了不存在文件的 SUMMARY 副本（临时目录）
  When 结构校验函数运行
  Then 返回的缺失清单指名该文件
