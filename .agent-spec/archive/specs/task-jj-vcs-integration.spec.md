spec: task
name: "jj VCS Integration"
inherits: project
tags: [done, vcs, jj, integration]
---

## Intent

Add optional jj version control awareness to agent-spec so that users who
colocate jj in their Git repos automatically get richer traceability—stable
change IDs in stamp trailers, jj operation IDs in run logs, and cross-run
file diffs in explain --history—without requiring jj as a dependency or
changing any default behaviour for pure-Git users.

## Decisions

- All jj interaction goes through `std::process::Command` calling the `jj` CLI binary; do not link jj-lib
- Detection is automatic: check for `.jj/` directory existence, no user configuration needed
- When jj is unavailable or not a jj repo, every code path silently falls back to existing Git behaviour
- VCS context logic lives in a single private module inside spec-cli; spec-core, spec-parser, spec-verify, spec-gateway are not modified
- RunLogEntry gains an optional `vcs` field with `#[serde(default, skip_serializing_if = "Option::is_none")]` so old log files remain readable

## Boundaries

### Allowed Changes
- src/main.rs
- src/vcs.rs (new file)
- Cargo.toml (only if a dev-dependency is needed for tests)

### Forbidden
- Do not modify spec-core, spec-parser, spec-lint, spec-verify, spec-gateway, or spec-report
- Do not add jj-lib or any jj Rust crate as a dependency
- Do not change the default behaviour of any existing command when jj is absent
- Do not require users to configure which VCS they use

## Completion Criteria

Scenario: VCS type auto-detection prefers jj in colocated repos
  Test:
    Package: agent-spec
    Filter: test_vcs_detect_prefers_jj_when_colocated
  Given a temporary directory initialised with both `git init` and `jj git init --colocate`
  When `detect_vcs_type` is called on that directory
  Then the result is `VcsType::Jj`

Scenario: VCS detection returns Git when only .git exists
  Test:
    Package: agent-spec
    Filter: test_vcs_detect_returns_git_when_only_git
  Given a temporary directory initialised with `git init` only
  When `detect_vcs_type` is called on that directory
  Then the result is `VcsType::Git`

Scenario: VCS detection returns None outside any repo
  Test:
    Package: agent-spec
    Filter: test_vcs_detect_returns_none_outside_repo
  Given an empty temporary directory with no .git or .jj
  When `detect_vcs_type` is called on that directory
  Then the result is `VcsType::None`

Scenario: get_vcs_context returns change ID and operation ID in jj repo
  Test:
    Package: agent-spec
    Filter: test_vcs_context_returns_jj_ids
  Given a temporary jj repo with at least one committed file
  When `get_vcs_context` is called
  Then the returned context has `vcs_type == Jj`
  And `change_ref` is a non-empty string
  And `operation_ref` is `Some` with a non-empty string

Scenario: get_vcs_context returns commit hash in pure Git repo
  Test:
    Package: agent-spec
    Filter: test_vcs_context_returns_git_hash
  Given a temporary Git repo with at least one commit
  When `get_vcs_context` is called
  Then the returned context has `vcs_type == Git`
  And `change_ref` is a non-empty short commit hash
  And `operation_ref` is `None`

Scenario: stamp dry-run includes Spec-Change trailer in jj repo
  Test:
    Package: agent-spec
    Filter: test_stamp_trailers_include_jj_change_id
  Given a jj repo is detected by `get_vcs_context`
  When `build_stamp_trailers` is called with VCS context
  Then the trailer list contains a line starting with `Spec-Change:`
  And the value is a non-empty jj change ID

Scenario: stamp dry-run omits Spec-Change trailer in pure Git repo
  Test:
    Package: agent-spec
    Filter: test_stamp_trailers_omit_change_id_for_git
  Given a pure Git repo is detected by `get_vcs_context`
  When `build_stamp_trailers` is called with VCS context
  Then no trailer line starts with `Spec-Change:`

Scenario: RunLogEntry serialises with optional VCS context
  Test:
    Package: agent-spec
    Filter: test_run_log_entry_serialises_vcs_context
  Given a RunLogEntry with a jj VcsContext attached
  When the entry is serialised to JSON and deserialised back
  Then the round-tripped entry contains the same vcs_type, change_ref, and operation_ref

Scenario: RunLogEntry without VCS context stays backward-compatible
  Test:
    Package: agent-spec
    Filter: test_run_log_entry_without_vcs_is_backward_compatible
  Given a JSON string from an old RunLogEntry without the vcs field
  When the string is deserialised into RunLogEntry
  Then the vcs field is None
  And all other fields parse correctly

Scenario: explain --history shows jj change diff between runs
  Test:
    Package: agent-spec
    Filter: test_explain_history_shows_jj_diff_between_runs
  Given two RunLogEntry records with different jj operation IDs
  When `read_run_log_history` formats the history
  And jj is available and `jj op diff` succeeds for those operation IDs
  Then the output includes a "Changes between runs" section
  And that section lists the modified file paths

Scenario: explain --history degrades gracefully without jj
  Test:
    Package: agent-spec
    Filter: test_explain_history_degrades_without_jj
  Given two RunLogEntry records with jj VCS context
  When `read_run_log_history` runs in an environment where `jj` is not on PATH
  Then the output still shows the run history
  And no "Changes between runs" section appears
  And no error is emitted

Scenario: existing change-scope jj still works end-to-end
  Test:
    Package: agent-spec
    Filter: test_resolve_command_change_paths_reads_jj_changes
  Given a jj repo with a modified file
  When agent-spec resolves change paths with `--change-scope jj`
  Then the modified file appears in the resolved list

## Out of Scope

- Contract-Change formal binding stored in jj metadata
- Conflict-aware verify after jj rebase
- jj checkpoint create/restore wrappers
- Linking jj-lib as a Rust dependency
- Any modification to jj itself
