# Spec Archive Summary

## Archived Specs

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

