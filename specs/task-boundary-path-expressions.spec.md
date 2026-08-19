spec: task
name: "Boundary Path Expressions"
tags: [boundaries, verification, parser, lint, bugfix]
satisfies: [REQ-BOUNDARY-PATH-EXPRESSIONS]
risk: B
estimate: 1d
---

## Intent

让 `### Allowed Changes` 条目按定义就是路径：去掉"看起来像路径"的后缀白名单，
裸根目录文件、任意扩展名、带反引号或尾注的条目都能成为边界模式；无法解析的
条目由 lint 指名而不是静默丢弃。同时把散落在 verifier、plan 与 MCP 的三份识别/
规范化/匹配逻辑收拢为一份，三处对同一条目与路径给出相同结论。来源是 robrix2
实践反馈 B1/B2（P0）。

## Decisions

- 新增 `src/spec_core/boundary_paths.rs` 作为唯一实现：
  `split_boundary_note(raw) -> (body, Option<note>)`、
  `normalize_boundary_pattern(raw) -> String`、`is_path_token(normalized) -> bool`、
  `collect_boundary_patterns(sections) -> (allowed, forbidden)`、
  `path_matches_pattern(pattern, path) -> bool`；verifier、plan、MCP 只调用它。
- 规范化顺序固定：剥尾注 → trim → 去首尾反引号 → `\` 换 `/` → 去前导 `./` → 去首尾 `/`。裸文件名即仓库根相对路径。
- 尾注只识别反引号跨度之外、前有空白的 ` — note`（em dash）与 ` # note`；`(…)` 一律不剥离。
- Allow 条目全部成为模式；规范化后为空、或含空白且首尾 token 之一不含路径字符
  （`/`、`.`、`*`）的 Allow 条目触发 Warning 级 `boundary-entry-shape`（`docs/foo (copy).md`
  两端仍是路径片段，不告警），消息含条目原文并建议 ` — note` 尾注或移入 Forbidden/Constraints。
- Deny 与 General 条目只在规范化后是单个路径 token（无空白且含 `/`、`*`、`.`、`?` 之一）时成为禁止模式；不从散文中抽取反引号路径。
- 段匹配语义保持现状（`*` 单段、`**` 跨段）；MCP `spec_allows_path` 改用同一匹配器与规范化，不再用未规范化的 `glob_match`。
- `TaskContract.allowed_changes` 保持原文用于展示；MCP `spec_allows_path` 在使用时规范化。

## Boundaries

### Allowed Changes
- src/spec_core/boundary_paths.rs
- src/spec_core/mod.rs
- src/spec_verify/boundaries.rs
- src/spec_gateway/plan.rs
- src/spec_mcp/tools.rs
- src/spec_lint/linters.rs
- src/spec_lint/pipeline.rs
- src/spec_lint/mod.rs
- specs/task-boundary-path-expressions.spec.md
- knowledge/requirements/req-boundary-path-expressions.md
- .agent-spec/wiki/**
- CHANGELOG.md
- skills/agent-spec-authoring/SKILL.md
- skills/agent-spec-authoring/references/patterns.md
- book/src/**

### Forbidden
- 不改变 `*` 与 `**` 的段匹配语义
- 不从自然语言禁令中抽取反引号路径作为禁止模式
- 不改变 `Symbols` 类别的处理
- 不修改 `check-structure` 的 glob 匹配器

## Out of Scope

- `--boundary-scope` / changed-spec 相关性过滤（队列 4）
- lifecycle 汇总把边界层与场景分列（A5）
- Forbidden 段中散文禁令的机械执行

## Completion Criteria

<!-- lint-ack: verification-metadata-suggestion — 本合约的主题就是路径模式，"path" 一词遍布场景；全部场景为进程内纯函数单元测试，不涉及外部 I/O -->

### Rule: allow-is-path — Allow 条目按定义即路径

场景: 裸根目录文件与任意扩展名被接受
  测试: test_boundary_allow_accepts_bare_root_files_and_any_extension
  假设 Allowed Changes 含 `Cargo.toml`、`` `CLAUDE.md` ``、`LICENSE`、`Makefile` 与 `tools/x/gate.json`
  当 变更集包含这五个文件并运行 boundaries verifier
  那么 五条 step verdict 均为 pass

场景: 未声明的根文件仍被拒绝
  测试: test_boundary_allow_still_rejects_undeclared_root_file
  假设 Allowed Changes 只含 `Cargo.toml`
  当 变更集包含 `Cargo.lock` 并运行 boundaries verifier
  那么 `Cargo.lock` 的 step verdict 为 fail 且 reason 为 "not covered by any allowed boundary"

### Rule: normalize — 规范化顺序与尾注

场景: 反引号、反斜杠与前导点斜杠按固定顺序规范化
  测试: test_normalize_boundary_pattern_strips_backticks_and_dot_slash
  假设 条目为 `` `./src\lib.rs` — note ``
  当 调用 normalize_boundary_pattern
  那么 按剥尾注、去反引号、换斜杠、去前导点斜杠的顺序得到 `src/lib.rs`

场景: 尾注被剥离而括号保留
  测试: test_boundary_note_split_keeps_parentheses
  假设 条目为 `` `Cargo.toml` — dev-dep only ``、`src/a.rs # new file` 与 `docs/foo (copy).md`
  当 调用 normalize_boundary_pattern
  那么 返回 `Cargo.toml`、`src/a.rs` 与 `docs/foo (copy).md`

场景: 反引号内的井号与破折号不是尾注
  测试: test_boundary_note_ignores_delimiters_inside_backticks
  假设 条目为 `` `docs/a — b.md` `` 与 `` `src/#tag.rs` ``
  当 调用 normalize_boundary_pattern
  那么 返回 `docs/a — b.md` 与 `src/#tag.rs`

### Rule: deny-token — 禁止模式只认整条路径 token

场景: 自然语言禁令不成为路径模式
  测试: test_boundary_deny_collects_only_path_tokens
  假设 Forbidden 含 "Do not modify `src/sliding_sync.rs`"、"不新增依赖" 与 `src/sliding_sync.rs`
  当 调用 collect_boundary_patterns
  那么 返回的禁止模式列表只包含 `src/sliding_sync.rs`

场景: 提到路径的禁令不误伤修改
  测试: test_boundary_deny_prose_mentioning_path_does_not_fail_change
  假设 Forbidden 只含 "Do not break the JSON shape in `src/spec_report/mod.rs`" 且无 Allowed Changes
  当 变更集包含 `src/spec_report/mod.rs` 并运行 boundaries verifier
  那么 该变更的 step verdict 为 pass

### Rule: shape-lint — 不像路径的 Allow 条目被指名

场景: 带括号说明的 Allow 条目被告警
  测试: test_boundary_entry_shape_warns_on_annotated_allow_entry
  假设 Allowed Changes 含 `` `Cargo.toml` (dev-dep only) ``
  当 运行 lint
  那么 输出 boundary-entry-shape Warning 且消息含 "(dev-dep only)" 与 " — "

场景: 合法路径与带尾注条目不告警
  测试: test_boundary_entry_shape_silent_on_valid_entries
  假设 Allowed Changes 含 `Cargo.toml`、`src/**`、`` `Cargo.toml` — dev-dep only `` 与 `docs/foo (copy).md`
  当 运行 lint
  那么 不输出 boundary-entry-shape 诊断

### Rule: single-source — 三处结论一致

场景: verifier、plan 与 MCP 对同一条目结论一致
  测试: test_boundary_matching_single_source_agrees_across_callers
  假设 一份 Allowed Changes 含 `./Cargo.toml` 与 `` `docs/*.md` — 说明 `` 的 spec 与变更 `Cargo.toml`、`docs/x.md`
  当 分别经 boundaries verifier、plan 的 allowed pattern 收集与 MCP spec_allows_path（对 `TaskContract.allowed_changes` 原文规范化后）判定
  那么 三处对两条变更的判定结果均为 allowed

场景: 旧白名单函数不再存在
  测试: test_boundary_no_local_looks_like_path_helpers_remain
  假设 仓库源码
  当 扫描 src/spec_verify/boundaries.rs 与 src/spec_gateway/plan.rs
  那么 两个文件都不包含 "fn looks_like_path"
