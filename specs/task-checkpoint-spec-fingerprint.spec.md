spec: task
name: "Checkpoint Spec Fingerprint"
tags: [lifecycle, resume, checkpoint, drift, bugfix]
satisfies: [REQ-CHECKPOINT-SPEC-FINGERPRINT]
risk: B
estimate: 0.5d
---

## Intent

让 `--resume` 不再把已改动 spec 的旧 pass 前移：checkpoint 记录 spec 内容指纹与
spec 名，恢复时任一不匹配就整体忽略 checkpoint 并给出诊断；匹配时行为不变。
run log 早已带 `spec_fingerprint`，这里把 checkpoint 对齐到同一算法。来源是
robrix2 反馈 D1，二次审查判为正确性问题前移到队列 2。

## Decisions

- `Checkpoint` 增加 `#[serde(default)] pub spec_fingerprint: String`；旧文件反序列化得到空串。
- 新增 `checkpoint_staleness(cp, spec_name, fingerprint) -> Option<String>`：spec_name 不同 →
  "spec name changed"；指纹为空 → "checkpoint predates spec fingerprinting"；指纹不同 →
  "spec content changed since checkpoint"；匹配返回 None。
- lifecycle 在加载 checkpoint 后立即调用它：Some(reason) → 丢弃 checkpoint（不进入
  `merge_checkpoint_results`）并记录 `checkpoint_diagnostic`；json 输出
  `{"severity":"warning","message":…}`，text 输出 `warning: checkpoint ignored: …` 到 stderr。
- `save_checkpoint_with_timestamp` 增加 `spec_fingerprint: &str` 参数，用
  `crate::spec_wiki::fingerprint_file(spec)` 的结果写入；lifecycle 提前计算一次指纹供两处复用。
- `merge_checkpoint_results` 本身不改；stale 判定在它之外。

## Boundaries

### Allowed Changes
- src/spec_core/verify.rs
- src/main.rs
- tests/lifecycle_resume.rs
- specs/task-checkpoint-spec-fingerprint.spec.md
- knowledge/requirements/req-checkpoint-spec-fingerprint.md
- .agent-spec/wiki/**
- CHANGELOG.md
- skills/agent-spec-tool-first/references/commands.md
- skills/agent-spec-tool-first/SKILL.md
- book/src/**

### Forbidden
- 不改变 `merge_checkpoint_results` 的合并逻辑
- 不改变 run log 的字段与指纹算法
- 不改变匹配时 incremental 与 conservative 的输出

## Out of Scope

- `explain --history` 标注 spec 内容变更（D1 其余部分）
- `stamp` 增加 `Spec-Fingerprint` trailer
- `--spec-frozen`

## Completion Criteria

### Rule: record — checkpoint 记录指纹

场景: 保存的 checkpoint 含指纹
  测试: test_checkpoint_records_spec_fingerprint
  假设 一份 verification report 与指纹 "abc123"
  当 调用 `save_checkpoint_with_timestamp` 后读回 checkpoint.json
  那么 反序列化得到的 spec_fingerprint 等于 "abc123"

场景: 旧格式 checkpoint 可加载且指纹为空
  测试: test_checkpoint_legacy_file_loads_with_empty_fingerprint
  假设 一份不含 spec_fingerprint 字段的 checkpoint.json
  当 调用 `load_checkpoint`
  那么 返回 Some 且 spec_fingerprint 为空串

### Rule: staleness — 过期判定

场景: 指纹不同判为过期
  测试: test_checkpoint_staleness_detects_content_change
  假设 checkpoint 指纹为 "old" 而当前指纹为 "new" 且 spec 名相同
  当 调用 `checkpoint_staleness`
  那么 返回 Some 且消息含 "spec content changed"

场景: spec 名不同判为过期
  测试: test_checkpoint_staleness_detects_name_change
  假设 checkpoint 的 spec_name 与当前不同而指纹相同
  当 调用 `checkpoint_staleness`
  那么 返回 Some 且消息含 "spec name changed"

场景: 旧格式判为过期
  测试: test_checkpoint_staleness_treats_missing_fingerprint_as_stale
  假设 checkpoint 指纹为空串
  当 调用 `checkpoint_staleness`
  那么 返回 Some 且消息含 "predates"

场景: 匹配时不过期
  测试: test_checkpoint_staleness_none_when_fresh
  假设 checkpoint 指纹与 spec 名都与当前一致
  当 调用 `checkpoint_staleness`
  那么 返回 None

### Rule: lifecycle — 过期 checkpoint 被忽略并有诊断

场景: spec 改动后旧 pass 不再前移
  测试: test_lifecycle_resume_ignores_stale_checkpoint
  假设 run-log-dir 里有一份 spec_name 匹配但指纹不同、场景 A 为 pass 的 checkpoint 且当前运行 A 为 skip
  当 以 --resume incremental --format json 运行 lifecycle
  那么 输出中场景 A 的 verdict 为 skip 且 checkpoint_diagnostic.message 含 "spec content changed"

场景: 新鲜 checkpoint 行为不变
  测试: test_lifecycle_resume_uses_fresh_checkpoint
  假设 run-log-dir 里有一份指纹与 spec 名都匹配、场景 A 为 pass 的 checkpoint 且当前运行 A 为 skip
  当 以 --resume incremental --format json 运行 lifecycle 并经 `merge_checkpoint_results` 合并
  那么 输出中场景 A 的 verdict 为 pass 且不含 checkpoint_diagnostic

场景: text 模式下过期 checkpoint 的警告写到 stderr
  测试: test_lifecycle_resume_stale_checkpoint_warns_on_stderr
  假设 run-log-dir 里有一份指纹不同的 checkpoint
  当 以 --resume incremental 且 text 格式运行 lifecycle
  那么 stderr 含 "checkpoint ignored" 且 stdout 不含该字样

场景: 损坏的 checkpoint 文件报错而不是静默忽略
  测试: test_lifecycle_resume_rejects_corrupt_checkpoint
  假设 run-log-dir 里的 checkpoint.json 不是合法 JSON
  当 以 --resume incremental 运行 lifecycle
  那么 命令返回错误且消息含 "checkpoint"
