# Spec Archive Summary

## Archived Specs

### AiVerifier 最小骨架

- Source: `specs/task-add-ai-verifier-skeleton.spec.md`
- Archive: `.agent-spec/archive/specs/task-add-ai-verifier-skeleton.spec.md`
- Satisfies: ``
- Depends: ``
- Tags: `done`, `bootstrap`, `verify`, `ai`, `gateway`, `report`, `phase4`
- Last verification: pass at 1786434309 (3/3 passed, 0 failed, 0 skipped, 0 uncertain)
- Scenarios:
  - stub 模式把未覆盖场景标成 uncertain
  - 文本报告输出 AI 证据
  - 默认 off 模式保持 skip 语义
- Test selectors:
  - test_format_verification_text_includes_ai_analysis_evidence
  - test_verify_default_keeps_uncovered_scenarios_skipped
  - test_verify_with_ai_mode_stub_marks_uncovered_scenarios_uncertain

### 新增行为完整性 lint

- Source: `specs/task-add-behavior-completeness-linters.spec.md`
- Archive: `.agent-spec/archive/specs/task-add-behavior-completeness-linters.spec.md`
- Satisfies: ``
- Depends: ``
- Tags: `done`, `contract-quality`, `lint`, `behavior`, `phase-next`
- Last verification: pass at 1786434312 (5/5 passed, 0 failed, 0 skipped, 0 uncertain)
- Scenarios:
  - 优先级和回退顺序未验证时给出提示
  - 可观察行为决策缺少场景时给出提示
  - 外部 I/O 错误场景的测试强度过弱时给出提示
  - 输出模式未覆盖时给出提示
  - 非行为性普通决策不会被新规则大量误报
- Test selectors:
  - test_behavior_completeness_linters_do_not_flag_plain_implementation_choices
  - test_external_io_error_strength_warns_on_weak_mock_only_http_scenarios
  - test_observable_decision_coverage_warns_when_behavioral_decisions_lack_scenarios
  - test_output_mode_coverage_warns_when_json_or_output_flags_are_uncovered
  - test_precedence_fallback_coverage_warns_when_ordered_behavior_has_no_scenario

### agent-spec audit v1：spec 库健康度扫描

- Source: `specs/task-audit-v1.spec.md`
- Archive: `.agent-spec/archive/specs/task-audit-v1.spec.md`
- Satisfies: ``
- Depends: `task-structural-check-v1`
- Tags: `done`, `audit`, `governance`, `phase8`
- Last verification: pass at 1786434317 (7/7 passed, 0 failed, 0 skipped, 0 uncertain)
- Scenarios:
  - audit JSON 可机器解析
  - 空库审计返回零计数不报错
  - 统计无归属 scenario
  - 统计未决 Discovery 问题
  - 统计未被证明的 Rule
  - 统计非法 Rule id
  - 聚合 spec/rule/scenario 计数
- Test selectors:
  - test_audit_counts_malformed_rules
  - test_audit_counts_open_questions
  - test_audit_counts_specs_rules_scenarios
  - test_audit_counts_ungrouped_scenarios
  - test_audit_counts_unproven_rules
  - test_audit_empty_library
  - test_audit_json_serializes

### BDD 语义增强 v1：Rule grouping + Scenario shape lint

- Source: `specs/task-bdd-semantics-v1.spec.md`
- Archive: `.agent-spec/archive/specs/task-bdd-semantics-v1.spec.md`
- Satisfies: ``
- Depends: ``
- Tags: `done`, `bdd`, `formulation`, `phase1`
- Last verification: pass at 1786434323 (19/19 passed, 0 failed, 0 skipped, 0 uncertain)
- Scenarios:
  - BehaviorRule 与 RuleScope 在 JSON 中序列化
  - Capability scope 是 reserved 类型
  - Example / 示例 / 例子 别名等价于 Scenario
  - JSON 输出只增不减
  - Rule 头部解析为 BehaviorRule
  - Rule 行只有 id 时 name 退回为 id
  - Rule 行缺少显式 kebab-case id 时触发 warning 且不归属
  - bdd-implementation-detail-step 识别中英文过程式动词
  - bdd-rule-grouping 对无 Rule 多场景提示
  - bdd-rule-grouping 警告空 Rule
  - bdd-scenario-shape 检查缺失 When / Then
  - bdd-scenario-shape 警告首步为 And / But / 并且 / 但是
  - contract 按 Rule 分组输出
  - plan --format prompt 在 Task Sketch 段包含 Rule 分组
  - 中文规则别名
  - 新 lint 不影响 lifecycle verdict
  - 新增 lint 诊断包含自我纠正指南四要素
  - 无 Rule 的旧 spec 兼容（verdict 与 contract/plan 格式不变；lint 可有非阻塞 info）
  - 现有 spec 在 v1 后的 guard verdict 完全不变（lint 输出可有新增 info）
- Test selectors:
  - test_bdd_implementation_detail_flags_ui_verbs_en_and_zh
  - test_bdd_rule_grouping_suggests_when_three_or_more_scenarios_uncategorized
  - test_bdd_rule_grouping_warns_on_empty_rule
  - test_bdd_scenario_shape_flags_leading_and_or_but
  - test_bdd_scenario_shape_flags_missing_when_or_then
  - test_capability_scope_is_reserved_in_v1
  - test_contract_renders_scenarios_grouped_by_rule
  - test_existing_specs_pass_lifecycle_after_v1_changes
  - test_freeform_rule_emits_warning_and_does_not_group_scenarios
  - test_json_output_additive_only
  - test_legacy_spec_without_rule_compat
  - test_new_bdd_lints_do_not_affect_lifecycle_verdict
  - test_new_bdd_lints_emit_self_correction_guidance
  - test_parse_chinese_rule_alias
  - test_parse_example_alias_as_scenario
  - test_parse_rule_header_creates_behavior_rule
  - test_parse_rule_header_without_display_name
  - test_plan_prompt_includes_rule_grouping
  - test_rule_scope_serializes_to_json

### Capability 层 + promote v1：长寿命真相库

- Source: `specs/task-capability-promote-v1.spec.md`
- Archive: `.agent-spec/archive/specs/task-capability-promote-v1.spec.md`
- Satisfies: ``
- Depends: `task-coverage-matrix-v1`
- Tags: `done`, `bdd`, `capability`, `promote`, `phase3`
- Last verification: pass at 1786434328 (12/12 passed, 0 failed, 0 skipped, 0 uncertain)
- Scenarios:
  - capability spec 的 Rule 带 Capability scope
  - promote 不改变 is_passing
  - spec capability 级别被解析
  - task 的 capability frontmatter 字段被解析
  - 事件日志可序列化与反序列化
  - 所有 Example 通过时 Rule 被提升进 capability spec
  - 提升不存在的 Rule id 报错
  - 新建 Rule 默认无事件且 JSON 不输出该键
  - 无 capability 字段时为 None 且 JSON 不输出该键
  - 有 Example 未通过时拒绝提升
  - 重复提升同一 Rule 幂等
  - 非法 spec 级别被拒绝
- Test selectors:
  - test_capability_spec_rule_has_capability_scope
  - test_parse_capability_spec_level
  - test_parse_task_capability_field
  - test_promote_appends_rule_when_examples_pass
  - test_promote_does_not_change_is_passing
  - test_promote_is_idempotent_for_same_rule
  - test_promote_refuses_when_an_example_fails
  - test_promote_unknown_rule_id_errors
  - test_rule_event_roundtrips
  - test_rule_events_additive_empty_by_default
  - test_task_without_capability_is_none_additive
  - test_unknown_spec_level_rejected

### 机械覆盖矩阵 v1：Rule × Scenario × Test × Verdict

- Source: `specs/task-coverage-matrix-v1.spec.md`
- Archive: `.agent-spec/archive/specs/task-coverage-matrix-v1.spec.md`
- Satisfies: ``
- Depends: `task-bdd-semantics-v1`
- Tags: `done`, `bdd`, `coverage-matrix`, `phase2`
- Last verification: pass at 1786434333 (15/15 passed, 0 failed, 0 skipped, 0 uncertain)
- Scenarios:
  - AI stub 的 uncertain 标为 inferential
  - caller-mode 经 resolve-ai 写回的结果标为 inferential
  - explain markdown 内嵌覆盖矩阵
  - json 格式可机器解析
  - markdown 格式渲染为表格
  - matrix 默认以 verify 默认模式运行
  - provenance 字段 JSON 只增不减
  - selector 指向不存在的测试记为 missing
  - test_found 要求精确函数名而非子串匹配
  - 无 selector 的 scenario 记为 none
  - 未分组 scenario 的 rule 列为占位符
  - 机械 verifier 的 verdict 标为 computational
  - 构建矩阵不改变 is_passing
  - 每个 scenario 一行且字段正确
  - 矩阵从 AiAnalysis 证据兜底派生 inferential
- Test selectors:
  - test_explain_markdown_embeds_coverage_matrix
  - test_json_provenance_additive_only
  - test_matrix_command_runs_verification_in_default_mode
  - test_matrix_derives_inferential_from_ai_evidence
  - test_matrix_does_not_change_is_passing
  - test_matrix_flags_dangling_selector_as_missing
  - test_matrix_has_one_row_per_scenario
  - test_matrix_json_is_machine_parseable
  - test_matrix_markdown_renders_table
  - test_matrix_marks_scenario_without_selector_as_none
  - test_matrix_test_found_requires_exact_function_name
  - test_matrix_ungrouped_scenario_rule_column_is_dash
  - test_provenance_ai_stub_is_inferential
  - test_provenance_resolve_ai_is_inferential
  - test_provenance_test_verifier_is_computational

### Guard 自动推导 staged change set

- Source: `specs/task-derive-change-set-from-staged-git-index.spec.md`
- Archive: `.agent-spec/archive/specs/task-derive-change-set-from-staged-git-index.spec.md`
- Satisfies: ``
- Depends: ``
- Tags: `done`, `bootstrap`, `cli`, `git`, `boundaries`, `guard`
- Last verification: pass at 1786434336 (3/3 passed, 0 failed, 0 skipped, 0 uncertain)
- Scenarios:
  - guard 从 staged git index 推导变更路径
  - 显式 change 参数优先于 git 自动发现
  - 非 git 目录保持空 change set
- Test selectors:
  - test_resolve_guard_change_paths_prefers_explicit_changes
  - test_resolve_guard_change_paths_reads_staged_git_changes
  - test_resolve_guard_change_paths_returns_empty_outside_git_repo

### discover --from-codebase v1：从测试反向生成 spec 骨架

- Source: `specs/task-discover-from-codebase-v1.spec.md`
- Archive: `.agent-spec/archive/specs/task-discover-from-codebase-v1.spec.md`
- Satisfies: ``
- Depends: `task-audit-v1`
- Tags: `done`, `discovery`, `cold-start`, `phase9`
- Last verification: pass at 1786434340 (5/5 passed, 0 failed, 0 skipped, 0 uncertain)
- Scenarios:
  - scenario 名来源于测试名
  - 每个测试函数生成一个带 selector 的 scenario
  - 生成的草案可被解析
  - 空测试集生成可解析的占位草案
  - 草案含 Questions 种子
- Test selectors:
  - test_draft_creates_scenario_per_test
  - test_draft_empty_tests_is_parseable
  - test_draft_includes_questions_seed
  - test_draft_is_parseable
  - test_draft_scenario_names_derive_from_tests

### Discovery v1：Questions 结构化产物

- Source: `specs/task-discovery-questions-v1.spec.md`
- Archive: `.agent-spec/archive/specs/task-discovery-questions-v1.spec.md`
- Satisfies: ``
- Depends: `task-capability-promote-v1`
- Tags: `done`, `bdd`, `discovery`, `phase4`
- Last verification: pass at 1786434344 (7/7 passed, 0 failed, 0 skipped, 0 uncertain)
- Scenarios:
  - Questions section 不进入 scenario 或 is_passing
  - Questions section 被解析
  - open-question 不改变 lint 通过性
  - 中文问题标题被识别
  - 已解决 question 不触发 warning
  - 未决 question 触发 open-question warning
  - 没有 Questions 的旧 spec 不受影响
- Test selectors:
  - test_open_question_is_non_gating
  - test_open_question_warns
  - test_parse_questions_section
  - test_parse_questions_section_chinese
  - test_questions_do_not_affect_verification
  - test_resolved_question_not_warned
  - test_spec_without_questions_unaffected

### 对显式 change set 执行边界校验

- Source: `specs/task-enforce-boundaries-with-explicit-change-set.spec.md`
- Archive: `.agent-spec/archive/specs/task-enforce-boundaries-with-explicit-change-set.spec.md`
- Satisfies: ``
- Depends: ``
- Tags: `done`, `bootstrap`, `verify`, `boundaries`, `contract`
- Last verification: pass at 1786434347 (3/3 passed, 0 failed, 0 skipped, 0 uncertain)
- Scenarios:
  - 允许范围内的显式变更通过边界校验
  - 命中禁止边界的显式变更失败
  - 超出允许范围的显式变更失败
- Test selectors:
  - test_boundaries_verifier_accepts_changes_within_allowed_paths
  - test_boundaries_verifier_rejects_change_matching_forbidden_boundary
  - test_boundaries_verifier_rejects_change_outside_allowed_paths

### 跳过场景不得判定为通过

- Source: `specs/task-fail-on-skipped.spec.md`
- Archive: `.agent-spec/archive/specs/task-fail-on-skipped.spec.md`
- Satisfies: ``
- Depends: ``
- Tags: `done`, `bootstrap`, `verify`, `lifecycle`
- Last verification: pass at 1786434348 (2/2 passed, 0 failed, 0 skipped, 0 uncertain)
- Scenarios:
  - 单个跳过场景导致非通过
  - 结构通过加跳过仍然非通过
- Test selectors:
  - test_pass_plus_skip_is_not_passing
  - test_skip_is_not_passing

### 修正 Contract 保真度

- Source: `specs/task-fix-contract-fidelity.spec.md`
- Archive: `.agent-spec/archive/specs/task-fix-contract-fidelity.spec.md`
- Satisfies: ``
- Depends: ``
- Tags: `done`, `bootstrap`, `contract`, `phase0`
- Last verification: pass at 1786434350 (3/3 passed, 0 failed, 0 skipped, 0 uncertain)
- Scenarios:
  - Task Contract 区分 Must 与 Decisions
  - contract 输出保留结构化验收信息
  - 继承链保留项目级约束与已定决策
- Test selectors:
  - test_contract_output_preserves_step_tables_and_test_selectors
  - test_load_resolves_full_project_contract_from_spec_directory
  - test_task_contract_keeps_must_must_not_and_decisions_distinct

### 修复继承链解析入口

- Source: `specs/task-fix-inheritance.spec.md`
- Archive: `.agent-spec/archive/specs/task-fix-inheritance.spec.md`
- Satisfies: ``
- Depends: ``
- Tags: `done`, `bootstrap`, `parser`, `gateway`, `cli`
- Last verification: pass at 1786434352 (3/3 passed, 0 failed, 0 skipped, 0 uncertain)
- Scenarios:
  - 内存入口保持原样
  - 同目录继承 project 规格
  - 磁盘入口不需要手工搜索路径
- Test selectors:
  - resolves_parent_from_source_directory_when_no_search_dirs_are_provided
  - test_full_lifecycle
  - test_load_resolves_inherited_constraints_from_spec_directory

### 正式化场景到测试的绑定

- Source: `specs/task-formalize-test-binding.spec.md`
- Archive: `.agent-spec/archive/specs/task-formalize-test-binding.spec.md`
- Satisfies: ``
- Depends: ``
- Tags: `done`, `bootstrap`, `verify`, `parser`, `contract`
- Last verification: pass at 1786434355 (3/3 passed, 0 failed, 0 skipped, 0 uncertain)
- Scenarios:
  - 场景可显式声明测试选择器
  - 旧版注释绑定继续兼容
  - 显式测试选择器优先于旧注释映射
- Test selectors:
  - test_explicit_scenario_selector_takes_precedence_over_legacy_comment_binding
  - test_legacy_comment_binding_is_used_when_no_explicit_selector_exists
  - test_parse_scenario_with_explicit_test_selector

### 单源多工具生成 v1

- Source: `specs/task-gen-integrations-v1.spec.md`
- Archive: `.agent-spec/archive/specs/task-gen-integrations-v1.spec.md`
- Satisfies: ``
- Depends: `task-lint-ack-dimensions-v1`
- Tags: `done`, `ecosystem`, `integrations`, `phase6`
- Last verification: pass at 1786434359 (7/7 passed, 0 failed, 0 skipped, 0 uncertain)
- Scenarios:
  - agents target 是纯 markdown 无 frontmatter
  - claude target 带 skill frontmatter
  - 三个 target 都包含同一核心正文
  - 内容一致时 check 通过
  - 内容不同时 check 报告漂移
  - 未知 target 报错
  - 正文提及 tool-first 工作流
- Test selectors:
  - test_agents_target_is_plain_markdown
  - test_all_targets_share_integration_body
  - test_check_passes_when_content_matches
  - test_check_reports_drift_when_different
  - test_claude_target_has_frontmatter
  - test_integration_body_is_tool_first
  - test_unknown_target_errors

### 宿主注入 AI backend

- Source: `specs/task-host-injected-ai-backend.spec.md`
- Archive: `.agent-spec/archive/specs/task-host-injected-ai-backend.spec.md`
- Satisfies: ``
- Depends: ``
- Tags: `done`, `bootstrap`, `ai`, `gateway`, `embed`, `phase4`
- Last verification: pass at 1786434361 (2/2 passed, 0 failed, 0 skipped, 0 uncertain)
- Scenarios:
  - gateway 支持注入自定义 AI backend
  - 默认 gateway 入口仍不依赖外部 provider
- Test selectors:
  - test_verify_default_keeps_uncovered_scenarios_skipped
  - test_verify_with_injected_ai_backend_uses_host_backend

### jj VCS Integration

- Source: `specs/task-jj-vcs-integration.spec.md`
- Archive: `.agent-spec/archive/specs/task-jj-vcs-integration.spec.md`
- Satisfies: ``
- Depends: ``
- Tags: `done`, `vcs`, `jj`, `integration`
- Last verification: pass at 1786434366 (12/12 passed, 0 failed, 0 skipped, 0 uncertain)
- Scenarios:
  - RunLogEntry serialises with optional VCS context
  - RunLogEntry without VCS context stays backward-compatible
  - VCS detection returns Git when only .git exists
  - VCS detection returns None outside any repo
  - VCS type auto-detection prefers jj in colocated repos
  - existing change-scope jj still works end-to-end
  - explain --history degrades gracefully without jj
  - explain --history shows jj change diff between runs
  - get_vcs_context returns change ID and operation ID in jj repo
  - get_vcs_context returns commit hash in pure Git repo
  - stamp dry-run includes Spec-Change trailer in jj repo
  - stamp dry-run omits Spec-Change trailer in pure Git repo
- Test selectors:
  - test_explain_history_degrades_without_jj
  - test_explain_history_shows_jj_diff_between_runs
  - test_resolve_command_change_paths_reads_jj_changes
  - test_run_log_entry_serialises_vcs_context
  - test_run_log_entry_without_vcs_is_backward_compatible
  - test_stamp_trailers_include_jj_change_id
  - test_stamp_trailers_omit_change_id_for_git
  - test_vcs_context_returns_git_hash
  - test_vcs_context_returns_jj_ids
  - test_vcs_detect_prefers_jj_when_colocated
  - test_vcs_detect_returns_git_when_only_git
  - test_vcs_detect_returns_none_outside_repo

### Lint-ack + 五维分类 v1

- Source: `specs/task-lint-ack-dimensions-v1.spec.md`
- Archive: `.agent-spec/archive/specs/task-lint-ack-dimensions-v1.spec.md`
- Satisfies: ``
- Depends: `task-discovery-questions-v1`
- Tags: `done`, `bdd`, `lint`, `governance`, `phase5`
- Last verification: pass at 1786434370 (8/8 passed, 0 failed, 0 skipped, 0 uncertain)
- Scenarios:
  - ack 不改变 lint 通过性
  - ack 不能抑制 Error 级诊断
  - 已知规则码映射到正确维度
  - 无 lint-ack 的旧 spec 字段为空且 JSON 不输出
  - 未知规则码有兜底维度
  - 未被 ack 的诊断不受影响
  - 被 ack 的 warning 移出主诊断
  - 解析 lint-ack 标记
- Test selectors:
  - test_ack_does_not_change_gating
  - test_dimension_of_known_rules
  - test_dimension_of_unknown_falls_back
  - test_lint_ack_cannot_suppress_error
  - test_lint_ack_leaves_other_diagnostics
  - test_lint_ack_moves_warning_to_acknowledged
  - test_lint_acks_additive_empty
  - test_parse_lint_ack_marker

### Task Contract 成为默认执行入口

- Source: `specs/task-make-contract-default.spec.md`
- Archive: `.agent-spec/archive/specs/task-make-contract-default.spec.md`
- Satisfies: ``
- Depends: ``
- Tags: `done`, `bootstrap`, `contract`, `gateway`, `cli`
- Last verification: pass at 1786434373 (2/2 passed, 0 failed, 0 skipped, 0 uncertain)
- Scenarios:
  - Brief 命令是 contract 兼容别名
  - Gateway 计划阶段返回 Task Contract
- Test selectors:
  - test_brief_output_matches_contract_output
  - test_plan_returns_task_contract

### AiVerifier 可插拔 backend 接口

- Source: `specs/task-pluggable-ai-backend-interface.spec.md`
- Archive: `.agent-spec/archive/specs/task-pluggable-ai-backend-interface.spec.md`
- Satisfies: ``
- Depends: ``
- Tags: `done`, `bootstrap`, `verify`, `ai`, `gateway`, `phase4`
- Last verification: pass at 1786434376 (3/3 passed, 0 failed, 0 skipped, 0 uncertain)
- Scenarios:
  - AI request 包含场景与代码上下文
  - AiVerifier 使用 backend 响应构造结果
  - Stub backend 返回结构化 AI 决策
- Test selectors:
  - test_ai_verifier_with_custom_backend_uses_backend_response
  - test_build_ai_request_includes_scenario_and_code_paths
  - test_stub_ai_backend_returns_uncertain_decision

### Probe 抽象 v1：Example 的可验证绑定泛化

- Source: `specs/task-probe-abstraction-v1.spec.md`
- Archive: `.agent-spec/archive/specs/task-probe-abstraction-v1.spec.md`
- Satisfies: ``
- Depends: `task-gen-integrations-v1`
- Tags: `done`, `bdd`, `probe`, `phase6_5`
- Last verification: pass at 1786434380 (6/6 passed, 0 failed, 0 skipped, 0 uncertain)
- Scenarios:
  - Probe 序列化往返
  - Scenario 结构未变(仍可无 probe 字段构造)
  - Test 探针标签为 test
  - 带 test_selector 的 scenario 派生出 Probe::Test
  - 无 test_selector 的 scenario 派生为 None
  - 预留变体有各自标签
- Test selectors:
  - test_probe_from_scenario_with_selector
  - test_probe_from_scenario_without_selector
  - test_probe_kind_label_reserved_variants
  - test_probe_kind_label_test
  - test_probe_roundtrips
  - test_scenario_unchanged_no_probe_field

### 缺少显式测试绑定时阻止通过

- Source: `specs/task-require-explicit-test-selectors.spec.md`
- Archive: `.agent-spec/archive/specs/task-require-explicit-test-selectors.spec.md`
- Satisfies: ``
- Depends: ``
- Tags: `done`, `bootstrap`, `lint`, `verify`, `quality-gate`
- Last verification: pass at 1786434382 (3/3 passed, 0 failed, 0 skipped, 0 uncertain)
- Scenarios:
  - error 级 lint 阻止质量闸门通过
  - 显式绑定的任务场景通过 lint
  - 缺少显式绑定的任务场景触发 lint 错误
- Test selectors:
  - test_explicit_test_binding_linter_accepts_explicit_selector
  - test_explicit_test_binding_linter_requires_task_scenario_selectors
  - test_quality_gate_fails_on_error_lint_issue

### 交付 Claude Code tool-first skills

- Source: `specs/task-ship-claude-code-tool-first-skills.spec.md`
- Archive: `.agent-spec/archive/specs/task-ship-claude-code-tool-first-skills.spec.md`
- Satisfies: ``
- Depends: ``
- Tags: `done`, `bootstrap`, `skills`, `claude-code`, `tool-first`, `phase5`
- Last verification: pass at 1786434384 (3/3 passed, 0 failed, 0 skipped, 0 uncertain)
- Scenarios:
  - README 说明 Claude Code skills 的使用方式
  - authoring skill 指向 Task Contract 写作
  - tool-first skill 指向核心 CLI 工作流
- Test selectors:
  - test_claude_code_authoring_skill_exists_and_mentions_task_contract_sections
  - test_claude_code_tool_first_skill_exists_and_mentions_contract_lifecycle_guard
  - test_readme_documents_claude_code_tool_first_skills

### 分阶段落盘后续改进路线图

- Source: `specs/task-stage-roadmap-specs.spec.md`
- Archive: `.agent-spec/archive/specs/task-stage-roadmap-specs.spec.md`
- Satisfies: ``
- Depends: ``
- Tags: `done`, `bootstrap`, `roadmap`, `planning`
- Last verification: pass at 1786434387 (4/4 passed, 0 failed, 0 skipped, 0 uncertain)
- Scenarios:
  - Phase 0 与 Phase 1 roadmap spec 已拆分并表达正确优先级
  - roadmap README 说明 staging 与 promotion 规则
  - 后续 roadmap spec 按 concern 分层
  - 嵌套 roadmap spec 继续继承顶层 project 规则
- Test selectors:
  - resolves_parent_from_nested_spec_directory_via_ancestor_specs_dir
  - test_roadmap_later_phase_specs_exist_and_are_split_by_concern
  - test_roadmap_phase_zero_and_one_specs_exist_and_capture_priorities
  - test_roadmap_readme_documents_promotion_rule

### StructuralRule v1：机械分层/禁止引用检查

- Source: `specs/task-structural-check-v1.spec.md`
- Archive: `.agent-spec/archive/specs/task-structural-check-v1.spec.md`
- Satisfies: ``
- Depends: `task-probe-abstraction-v1`
- Tags: `done`, `probe`, `structural`, `phase7`
- Last verification: pass at 1786434389 (5/5 passed, 0 failed, 0 skipped, 0 uncertain)
- Scenarios:
  - glob 之外的文件不被检查
  - 匹配文件含禁止子串时报违规
  - 无违规时返回空
  - 有违规时报告非空
  - 跳过 target 目录
- Test selectors:
  - test_check_structure_reports_violations
  - test_structural_flags_forbidden_reference
  - test_structural_no_violation_returns_empty
  - test_structural_respects_glob_scope
  - test_structural_skips_target_dir

### 结构化测试选择器

- Source: `specs/task-structure-test-selectors.spec.md`
- Archive: `.agent-spec/archive/specs/task-structure-test-selectors.spec.md`
- Satisfies: ``
- Depends: ``
- Tags: `done`, `bootstrap`, `verify`, `parser`, `contract`, `phase4`
- Last verification: pass at 1786434391 (3/3 passed, 0 failed, 0 skipped, 0 uncertain)
- Scenarios:
  - parser 保留结构化测试选择器
  - verifier 使用 package 范围执行测试
  - 单行测试选择器继续兼容
- Test selectors:
  - test_build_cargo_test_command_with_package_selector
  - test_parse_shorthand_test_selector_as_filter_only
  - test_parse_structured_test_selector_block

### verify 与 lifecycle 支持可选 change scope

- Source: `specs/task-support-change-scope-in-verify-and-lifecycle.spec.md`
- Archive: `.agent-spec/archive/specs/task-support-change-scope-in-verify-and-lifecycle.spec.md`
- Satisfies: ``
- Depends: ``
- Tags: `done`, `bootstrap`, `cli`, `git`, `boundaries`, `lifecycle`, `verify`, `phase4`
- Last verification: pass at 1786434394 (3/3 passed, 0 failed, 0 skipped, 0 uncertain)
- Scenarios:
  - lifecycle 在 worktree scope 下读取整棵工作区变更
  - verify 默认 none scope 保持空 change set
  - 显式 change 参数继续优先于自动 scope
- Test selectors:
  - test_resolve_command_change_paths_prefers_explicit_changes
  - test_resolve_command_change_paths_reads_worktree_git_changes
  - test_resolve_command_change_paths_returns_empty_for_none_scope

### Guard 支持 git worktree change scope

- Source: `specs/task-support-git-worktree-change-scope.spec.md`
- Archive: `.agent-spec/archive/specs/task-support-git-worktree-change-scope.spec.md`
- Satisfies: ``
- Depends: ``
- Tags: `done`, `bootstrap`, `cli`, `git`, `boundaries`, `guard`, `phase4`
- Last verification: pass at 1786434398 (3/3 passed, 0 failed, 0 skipped, 0 uncertain)
- Scenarios:
  - worktree scope 包含 staged、未暂存和未跟踪文件
  - 显式 change 参数优先于 scope 自动发现
  - 默认 staged scope 不包含未暂存改动
- Test selectors:
  - test_resolve_guard_change_paths_ignores_unstaged_changes_in_default_staged_scope
  - test_resolve_guard_change_paths_prefers_explicit_changes
  - test_resolve_guard_change_paths_reads_worktree_git_changes

### 支持 .spec.md 双扩展名

- Source: `specs/task-support-spec-md-extension.spec.md`
- Archive: `.agent-spec/archive/specs/task-support-spec-md-extension.spec.md`
- Satisfies: ``
- Depends: ``
- Tags: `done`, `enhancement`, `ux`, `backward-compat`
- Last verification: pass at 1786434403 (10/10 passed, 0 failed, 0 skipped, 0 uncertain)
- Scenarios:
  - boundary checker 识别 .spec.md 为 spec 路径
  - guard 发现 .spec.md 文件
  - guard 同时发现 .spec 和 .spec.md 文件
  - init 默认生成 .spec.md 文件
  - resolver 优先查找 .spec.md 继承文件
  - resolver 回退到 .spec 继承文件
  - resolver 找不到继承文件时报错
  - 同名 .spec 和 .spec.md 共存时 guard 警告
  - 文件发现使用 file_name 判断而非 Path::extension
  - 非 spec 的 .md 文件不被误识别
- Test selectors:
  - test_boundary_checker_recognizes_spec_md
  - test_guard_discovers_both_spec_and_spec_md
  - test_guard_discovers_spec_md_files
  - test_init_creates_spec_md_by_default
  - test_lint_warns_on_duplicate_spec_extensions
  - test_plain_md_files_not_matched_as_spec
  - test_resolver_errors_when_no_spec_or_spec_md_found
  - test_resolver_falls_back_to_spec_when_no_spec_md
  - test_resolver_prefers_spec_md_over_spec
  - test_spec_md_not_matched_by_extension_alone

### 保留步骤表格输入

- Source: `specs/task-support-step-tables.spec.md`
- Archive: `.agent-spec/archive/specs/task-support-step-tables.spec.md`
- Satisfies: ``
- Depends: ``
- Tags: `done`, `bootstrap`, `parser`, `dsl`
- Last verification: pass at 1786434406 (3/3 passed, 0 failed, 0 skipped, 0 uncertain)
- Scenarios:
  - JSON 输出保留表格
  - When 步骤携带请求表格
  - 无表格场景保持兼容
- Test selectors:
  - test_parse_scenario_without_table_stays_unchanged
  - test_parse_step_table_and_preserve_json_output

## Orphan Baseline Retirement Disposition

Thirty retired contracts are recorded above with their original active source
paths and current passing lifecycle evidence. The six active paths below were
duplicates of pre-existing archive contracts: after accounting for the
archive-only `done` tag, their contract bytes were identical. Their original
passing lifecycle records remain in the preserved entries below.

- `specs/task-phase1-contract-review-loop.spec.md` → `.agent-spec/archive/specs/task-phase1-contract-review-loop.spec.md` (3/3 pass at 1783793703)
- `specs/task-phase2-run-history-and-vcs-context.spec.md` → `.agent-spec/archive/specs/task-phase2-run-history-and-vcs-context.spec.md` (3/3 pass at 1783793704)
- `specs/task-phase3-spec-governance.spec.md` → `.agent-spec/archive/specs/task-phase3-spec-governance.spec.md` (3/3 pass at 1783793705)
- `specs/task-phase4-ai-verification-expansion.spec.md` → `.agent-spec/archive/specs/task-phase4-ai-verification-expansion.spec.md` (3/3 pass at 1783793706)
- `specs/task-phase5-ecosystem-integrations.spec.md` → `.agent-spec/archive/specs/task-phase5-ecosystem-integrations.spec.md` (3/3 pass at 1783793707)
- `specs/task-phase6-advanced-verification.spec.md` → `.agent-spec/archive/specs/task-phase6-advanced-verification.spec.md` (3/3 pass at 1783793707)

## Preserved Earlier Archive Evidence

The following entries predate this retirement batch. They remain here because
the archive command's batch summary must not erase previously recorded passing
evidence.

### Phase 1：Contract Review Loop

- Source: `specs/roadmap/task-phase1-contract-review-loop.spec.md`
- Archive: `.agent-spec/archive/specs/task-phase1-contract-review-loop.spec.md`
- Satisfies: ``
- Depends: ``
- Tags: `done`, `roadmap`, `planned`, `phase1`, `review`
- Last verification: pass at 1783793703 (3/3 passed, 0 failed, 0 skipped, 0 uncertain)
- Scenarios:
  - explain 生成 PR description markdown
  - explain 生成人类可读的 Contract 摘要
  - stamp 默认安全且支持预览
- Test selectors:
  - test_explain_command_renders_contract_review_summary
  - test_explain_markdown_output_is_suitable_for_pr_description
  - test_stamp_dry_run_outputs_trailers_without_rewriting_history

### Phase 2：Run History 与 VCS Context

- Source: `specs/roadmap/task-phase2-run-history-and-vcs-context.spec.md`
- Archive: `.agent-spec/archive/specs/task-phase2-run-history-and-vcs-context.spec.md`
- Satisfies: ``
- Depends: ``
- Tags: `done`, `roadmap`, `planned`, `phase2`, `traceability`
- Last verification: pass at 1783793704 (3/3 passed, 0 failed, 0 skipped, 0 uncertain)
- Scenarios:
  - explain 展示执行历史
  - lifecycle 可记录结构化 run log
  - 命令行支持 jj change scope
- Test selectors:
  - test_explain_history_reads_run_log_summary
  - test_lifecycle_writes_structured_run_log_summary
  - test_resolve_command_change_paths_reads_jj_changes

### Phase 3：Spec Governance

- Source: `specs/roadmap/task-phase3-spec-governance.spec.md`
- Archive: `.agent-spec/archive/specs/task-phase3-spec-governance.spec.md`
- Satisfies: ``
- Depends: ``
- Tags: `done`, `roadmap`, `planned`, `phase3`, `governance`
- Last verification: pass at 1783793705 (3/3 passed, 0 failed, 0 skipped, 0 uncertain)
- Scenarios:
  - lint 报告 Spec 质量
  - lint 检测跨 spec 机械矛盾
  - org.spec 参与三层继承链
- Test selectors:
  - test_cross_check_reports_boundary_and_decision_conflicts
  - test_load_resolves_org_project_task_chain
  - test_quality_report_scores_testability_and_smells

### Phase 4：AI Verification Expansion

- Source: `specs/roadmap/task-phase4-ai-verification-expansion.spec.md`
- Archive: `.agent-spec/archive/specs/task-phase4-ai-verification-expansion.spec.md`
- Satisfies: ``
- Depends: ``
- Tags: `done`, `roadmap`, `planned`, `phase4`, `ai`
- Last verification: pass at 1783793706 (3/3 passed, 0 failed, 0 skipped, 0 uncertain)
- Scenarios:
  - AI request 打包完整验证上下文
  - adversarial 验证保持显式 opt-in
  - lint 检测 sycophancy 风险
- Test selectors:
  - test_adversarial_verification_is_disabled_by_default
  - test_build_ai_request_includes_contract_change_set_and_evidence_context
  - test_sycophancy_linter_flags_bug_finding_bias

### Phase 5：Ecosystem Integrations

- Source: `specs/roadmap/task-phase5-ecosystem-integrations.spec.md`
- Archive: `.agent-spec/archive/specs/task-phase5-ecosystem-integrations.spec.md`
- Satisfies: ``
- Depends: ``
- Tags: `done`, `roadmap`, `planned`, `phase5`, `ecosystem`
- Last verification: pass at 1783793707 (3/3 passed, 0 failed, 0 skipped, 0 uncertain)
- Scenarios:
  - JSON 输出适合作为编排接口
  - checkpoint 能力保持可选
  - 提供更多 Agent 工具的集成模板
- Test selectors:
  - test_additional_agent_integration_templates_exist
  - test_checkpoint_commands_are_optional_and_vcs_aware
  - test_report_json_exposes_contract_and_verification_summary_for_orchestrators

### Phase 6：Advanced Verification

- Source: `specs/roadmap/task-phase6-advanced-verification.spec.md`
- Archive: `.agent-spec/archive/specs/task-phase6-advanced-verification.spec.md`
- Satisfies: ``
- Depends: ``
- Tags: `done`, `roadmap`, `planned`, `phase6`, `verification`
- Last verification: pass at 1783793707 (3/3 passed, 0 failed, 0 skipped, 0 uncertain)
- Scenarios:
  - lifecycle 支持显式验证层选择
  - 成本报告按层输出
  - 确定性度量保持实验功能
- Test selectors:
  - test_cost_report_breaks_down_tokens_time_and_layers
  - test_lifecycle_layers_flag_selects_verification_stack
  - test_measure_determinism_is_explicitly_experimental

### 输出保真度分级（Context Fidelity）

- Source: `specs/roadmap/task-context-fidelity.spec.md`
- Archive: `.agent-spec/archive/specs/task-context-fidelity.spec.md`
- Satisfies: ``
- Depends: ``
- Tags: `done`, `bootstrap`, `lifecycle`, `report`, `phase7`
- Last verification: pass at 1783847157 (3/3 passed, 0 failed, 0 skipped, 0 uncertain)
- Scenarios:
  - compact 格式输出单行摘要
  - diagnostic 格式包含测试原始输出
  - 现有 json 格式不受影响
- Test selectors:
  - test_compact_format_outputs_single_line_summary
  - test_diagnostic_format_includes_raw_test_output
  - test_existing_json_format_unchanged

### 运行历史汇总视图

- Source: `specs/roadmap/task-history-summary.spec.md`
- Archive: `.agent-spec/archive/specs/task-history-summary.spec.md`
- Satisfies: ``
- Depends: `task-context-fidelity`
- Tags: `done`, `bootstrap`, `lifecycle`, `report`, `phase8`
- Last verification: pass at 1783847158 (4/4 passed, 0 failed, 0 skipped, 0 uncertain)
- Scenarios:
  - delta 列显示与前次差异
  - history 支持 JSON 格式输出
  - history 输出表格化汇总
  - 单次运行时 delta 为空
- Test selectors:
  - test_history_delta_shows_diff_from_previous
  - test_history_json_format_output
  - test_history_outputs_tabular_summary
  - test_history_single_run_no_delta

### 标准化状态文件协议（Status File Contract）

- Source: `specs/roadmap/task-status-file-contract.spec.md`
- Archive: `.agent-spec/archive/specs/task-status-file-contract.spec.md`
- Satisfies: ``
- Depends: `task-goal-gate`
- Tags: `done`, `bootstrap`, `lifecycle`, `report`, `phase7`
- Last verification: pass at 1783847159 (4/4 passed, 0 failed, 0 skipped, 0 uncertain)
- Scenarios:
  - gate_blocked 时 outcome 反映门禁状态
  - 全部通过时写入 success 状态
  - 无 --status-file 时不产生文件
  - 部分失败时写入 partial_success 状态
- Test selectors:
  - test_no_status_file_flag_produces_no_file
  - test_status_file_outcome_reflects_gate_blocked
  - test_status_file_writes_partial_success_on_mixed
  - test_status_file_writes_success_on_all_pass

### 强化 rewrite/parity 合同写作

- Source: `specs/roadmap/task-strengthen-rewrite-contract-authoring.spec.md`
- Archive: `.agent-spec/archive/specs/task-strengthen-rewrite-contract-authoring.spec.md`
- Satisfies: ``
- Depends: ``
- Tags: `done`, `contract-quality`, `skills`, `templates`, `parity`, `phase-next`
- Last verification: pass at 1783847160 (6/6 passed, 0 failed, 0 skipped, 0 uncertain)
- Scenarios:
  - README 说明 rewrite/parity 合同的写法与普通功能合同不同
  - authoring skill 包含行为面检查清单
  - skill 不会把普通功能合同误判为 parity 合同
  - skill 明确指出遗漏行为矩阵时合同不应交付
  - tool-first skill 包含未绑定可观察行为审查步骤
  - 仓库提供 rewrite/parity 示例合同
- Test selectors:
  - test_authoring_skill_includes_behavior_surface_checklist
  - test_readme_documents_rewrite_parity_contract_authoring_guidance
  - test_rewrite_parity_example_spec_exists_and_covers_behavior_matrix
  - test_skill_guidance_does_not_require_behavior_matrix_for_non_parity_tasks
  - test_skill_guidance_rejects_parity_contracts_missing_behavior_matrix
  - test_tool_first_skill_mentions_unbound_observable_behavior_review_step
