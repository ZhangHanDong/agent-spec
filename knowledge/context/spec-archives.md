# Spec Archive Summary

## Archived Specs

### 检查点与增量重跑（Checkpoint/Resume）

- Source: `specs/roadmap/task-checkpoint-resume.spec.md`
- Archive: `.agent-spec/archive/specs/task-checkpoint-resume.spec.md`
- Satisfies: ``
- Depends: `task-goal-gate`, `task-context-fidelity`
- Tags: `done`, `bootstrap`, `lifecycle`, `verify`, `phase8`
- Last verification: pass at 1783793699 (4/4 passed, 0 failed, 0 skipped, 0 uncertain)
- Scenarios:
  - checkpoint 文件可序列化和反序列化
  - 保守模式检测回归
  - 增量模式跳过已通过场景
  - 无 run-log-dir 时 resume 报错
- Test selectors:
  - test_checkpoint_roundtrip_serialization
  - test_resume_conservative_detects_regression
  - test_resume_incremental_skips_passed_scenarios
  - test_resume_without_run_log_dir_errors

### 代码质量门禁（Complexity Gate）

- Source: `specs/roadmap/task-complexity-gate.spec.md`
- Archive: `.agent-spec/archive/specs/task-complexity-gate.spec.md`
- Satisfies: ``
- Depends: `task-goal-gate`
- Tags: `done`, `bootstrap`, `verify`, `phase8`
- Last verification: pass at 1783793700 (4/4 passed, 0 failed, 0 skipped, 0 uncertain)
- Scenarios:
  - 使用 git diff 统计行数变化
  - 无质量约束时无 verdict
  - 行数比超标时 fail
  - 行数比达标时 pass
- Test selectors:
  - test_complexity_verifier_fails_on_line_ratio_exceeded
  - test_complexity_verifier_passes_on_acceptable_ratio
  - test_complexity_verifier_silent_without_constraints
  - test_complexity_verifier_uses_git_diff_stats

### 关键场景门禁（Goal Gate）

- Source: `specs/roadmap/task-goal-gate.spec.md`
- Archive: `.agent-spec/archive/specs/task-goal-gate.spec.md`
- Satisfies: ``
- Depends: ``
- Tags: `done`, `bootstrap`, `lifecycle`, `verify`, `phase7`
- Last verification: pass at 1783793700 (5/5 passed, 0 failed, 0 skipped, 0 uncertain)
- Scenarios:
  - critical 场景失败时报告 gate_blocked
  - critical 场景通过时不触发门禁
  - critical 失败的退出码为 2
  - 场景名称后缀作为 critical 简写
  - 无 critical 标签时行为不变
- Test selectors:
  - test_critical_fail_exit_code_is_2
  - test_critical_scenario_fail_sets_gate_blocked
  - test_critical_scenario_pass_no_gate_block
  - test_critical_suffix_in_scenario_name
  - test_no_critical_tag_preserves_existing_behavior

### 人类审核场景（Human Review）

- Source: `specs/roadmap/task-human-review.spec.md`
- Archive: `.agent-spec/archive/specs/task-human-review.spec.md`
- Satisfies: ``
- Depends: `task-complexity-gate`
- Tags: `done`, `bootstrap`, `verify`, `parser`, `phase9`
- Last verification: pass at 1783793701 (4/4 passed, 0 failed, 0 skipped, 0 uncertain)
- Scenarios:
  - auto 模式下 pending_review 视为通过
  - human 审核场景测试通过后为 pending_review
  - parser 正确解析审核字段
  - strict 模式下 pending_review 为非通过
- Test selectors:
  - test_auto_review_mode_treats_pending_as_pass
  - test_human_review_scenario_produces_pending_review
  - test_parse_review_field_in_scenario
  - test_strict_review_mode_treats_pending_as_not_pass

### 开放式优化场景模式

- Source: `specs/roadmap/task-optimize-scenario-mode.spec.md`
- Archive: `.agent-spec/archive/specs/task-optimize-scenario-mode.spec.md`
- Satisfies: ``
- Depends: `task-checkpoint-resume`
- Tags: `done`, `bootstrap`, `parser`, `lifecycle`, `phase9`
- Last verification: pass at 1783793702 (3/3 passed, 0 failed, 0 skipped, 0 uncertain)
- Scenarios:
  - optimize 场景 fail 不影响 passed 判定
  - optimize 场景 pass 后出现在 optimization_candidates
  - parser 正确解析模式字段
- Test selectors:
  - test_optimize_scenario_fail_blocks_pass
  - test_optimize_scenario_pass_listed_as_candidate
  - test_parse_mode_field_in_scenario

### Phase 0：Contract 保真度修正

- Source: `specs/roadmap/task-phase0-contract-fidelity.spec.md`
- Archive: `.agent-spec/archive/specs/task-phase0-contract-fidelity.spec.md`
- Satisfies: ``
- Depends: ``
- Tags: `done`, `roadmap`, `planned`, `phase0`, `contract`
- Last verification: pass at 1783793703 (3/3 passed, 0 failed, 0 skipped, 0 uncertain)
- Scenarios:
  - Task Contract 区分 Must 与 Decisions
  - contract 输出保留结构化验收信息
  - 继承链保留项目级约束与已定决策
- Test selectors:
  - test_contract_output_preserves_step_tables_and_test_selectors
  - test_load_resolves_full_project_contract_from_spec_directory
  - test_task_contract_keeps_must_must_not_and_decisions_distinct

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

### 实现计划生成（Plan Command）

- Source: `specs/roadmap/task-plan-command.spec.md`
- Archive: `.agent-spec/archive/specs/task-plan-command.spec.md`
- Satisfies: ``
- Depends: `task-spec-dependency-graph`
- Tags: `done`, `bootstrap`, `lifecycle`, `phase9`
- Last verification: pass at 1783793708 (9/9 passed, 0 failed, 0 skipped, 0 uncertain)
- Scenarios:
  - Allowed Changes 路径不存在时输出警告
  - plan --depth full 输出 pub API 签名
  - plan --format json 输出可解析的 JSON
  - plan --format prompt 输出 self-contained prompt
  - plan 扫描尊重 gitignore
  - plan 输出包含 Codebase Context 区块
  - plan 输出包含 Contract 区块（critical）
  - plan 输出包含 Task Sketch 区块
  - plan 输出测试文件中的 test 函数名列表
- Test selectors:
  - test_plan_full_depth_includes_pub_signatures
  - test_plan_includes_codebase_context
  - test_plan_includes_contract_section
  - test_plan_includes_task_sketch
  - test_plan_json_format_is_valid
  - test_plan_lists_existing_test_functions
  - test_plan_prompt_format_is_self_contained
  - test_plan_respects_gitignore
  - test_plan_warns_on_missing_boundary_path

### 场景依赖与拓扑排序执行

- Source: `specs/roadmap/task-scenario-dependencies.spec.md`
- Archive: `.agent-spec/archive/specs/task-scenario-dependencies.spec.md`
- Satisfies: ``
- Depends: `task-checkpoint-resume`, `task-history-summary`
- Tags: `done`, `bootstrap`, `parser`, `lifecycle`, `lint`, `phase9`
- Last verification: pass at 1783793709 (5/5 passed, 0 failed, 0 skipped, 0 uncertain)
- Scenarios:
  - parser 正确解析前置字段
  - 前置失败时依赖场景被跳过
  - 循环依赖被 lint 检测
  - 拓扑排序保证执行顺序
  - 无依赖声明时执行顺序不变
- Test selectors:
  - test_dependency_skip_on_prerequisite_fail
  - test_lint_detects_circular_dependency
  - test_no_dependency_preserves_original_order
  - test_parse_depends_field_in_scenario
  - test_topological_sort_execution_order

### Spec 依赖图与 DOT 可视化

- Source: `specs/roadmap/task-spec-dependency-graph.spec.md`
- Archive: `.agent-spec/archive/specs/task-spec-dependency-graph.spec.md`
- Satisfies: ``
- Depends: ``
- Tags: `done`, `bootstrap`, `cli`, `planning`, `phase7`
- Last verification: pass at 1783793710 (5/5 passed, 0 failed, 0 skipped, 0 uncertain)
- Scenarios:
  - DOT 节点包含工作量估算
  - frontmatter 解析 depends 和 estimate
  - 关键路径标记
  - 无依赖的 spec 作为独立节点
  - 生成 DOT 依赖图
- Test selectors:
  - test_graph_critical_path_highlighted
  - test_graph_generates_dot_output
  - test_graph_independent_specs_are_isolated_nodes
  - test_graph_nodes_include_estimate
  - test_parse_spec_depends_and_estimate_fields

### 支持场景验证强度元数据

- Source: `specs/roadmap/task-support-scenario-verification-metadata.spec.md`
- Archive: `.agent-spec/archive/specs/task-support-scenario-verification-metadata.spec.md`
- Satisfies: ``
- Depends: ``
- Tags: `done`, `dsl`, `parser`, `lint`, `verification`, `phase-next`
- Last verification: pass at 1783793711 (5/5 passed, 0 failed, 0 skipped, 0 uncertain)
- Scenarios:
  - JSON 输出与 contract 渲染保留元数据
  - parser 同时支持英文验证强度元数据关键字
  - parser 解析中文验证强度元数据并保留到 AST
  - 旧规格文件保持兼容
  - 高风险 I/O 场景缺少元数据时得到建议
- Test selectors:
  - test_contract_and_json_output_preserve_verification_metadata
  - test_existing_specs_without_verification_metadata_remain_valid
  - test_lint_suggests_verification_metadata_for_external_io_scenarios
  - test_parse_english_verification_metadata_fields
  - test_parse_scenario_verification_metadata_fields

