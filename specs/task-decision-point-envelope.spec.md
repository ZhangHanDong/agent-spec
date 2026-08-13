spec: task
name: "Decision Point Envelope"
tags: [knowledge, questions, interop]
satisfies: [REQ-DECISION-POINT-ENVELOPE]
risk: B
---

## Intent

把 `ClarificationQuestion` 从「有 options 字段但永远为空」升级成三个阶段
共享的决策点信封：结构化候选、阶段标记、形状校验、版本字段。候选由 agent
起草，本合约只负责信封类型与校验器，不含发射点接线。

## Decisions

- 候选类型 `DecisionOption { label, description, value }`，替换
  `options: Vec<String>`；`value` 为回填时使用的值。
- `ClarificationQuestion` 增 `kind: QuestionKind`（requirements | knowledge
  | verification）与 `multi_select: bool`，其余字段语义不变。
- questions JSON 顶层增 `envelope_version: u32`，起始值 1。
- 校验器 `validate_envelope` 返回诊断列表而非 bool：超过四项候选、缺 label、
  缺 description 各自一条诊断，诊断文本含问题 id 与违规字段名。
- 空候选表合法：校验器对空 options 返回零诊断。
- 问题增可选 `answer`（`AiDecision` 形状）与可选 `scenario_name`；信封增可选
  `verification_context`（ai_mode 与变更路径）。三者均 `skip_serializing_if`，
  非验证类问题与未回答问题的线格式保持 v1 不变。
- 验证类问题 id 形如 `Q-VERIFY-<序号>-<可读 slug>-<场景名 blake3 前 12 位>`：
  可读部分方便人看，digest 保证长前缀相同的场景名不撞 id。
- 证据在进入信封前归一化：剔除 cargo 编译进度、Finished 计时与 deps 哈希路径
  等与判定无关的行，使冷热两次运行产出相同证据——否则逐字段绑定校验会因
  构建噪音误报陈旧。

## Boundaries

### Allowed Changes
- src/spec_knowledge/questions.rs
- src/spec_knowledge/mod.rs
- src/main.rs
- specs/task-decision-point-envelope.spec.md

### Forbidden
- 不在本合约内接线任何发射点（emission 合约负责）
- 不在 CLI 内生成候选内容
- 不移除自由作答路径的语义

## Out of Scope

- `knowledge questions` 与 `verify --emit-questions` 命令
- 判定进 provenance
- 回填文档的实现

## Completion Criteria

场景: 信封携带阶段与结构化候选
  测试: test_envelope_carries_kind_and_structured_options
  假设 一个由 requirements 阶段构造的问题带两项候选
  当 序列化为 json
  那么 输出含 kind 字段与每项带 label 和 description 的候选

场景: 超过四项候选被拒绝
  测试: test_envelope_rejects_more_than_four_options
  假设 一个带五项候选的信封
  当 validate_envelope 运行
  那么 返回诊断含该问题 id 与候选数上限

场景: 缺 description 的候选被指名
  测试: test_envelope_rejects_option_without_description
  假设 一项只有 label 的候选
  当 validate_envelope 运行
  那么 返回诊断指名 description 字段

场景: 空候选表合法通过
  测试: test_envelope_accepts_empty_options
  假设 一个候选表为空的信封
  当 validate_envelope 运行
  那么 返回零诊断

场景: 版本字段随 json 输出
  测试: test_questions_json_carries_envelope_version
  假设 任意 questions 输出
  当 以 json 格式序列化
  那么 顶层含 envelope_version 且值为 1

场景: 候选缺 label 被指名
  测试: test_envelope_rejects_option_without_label
  假设 一项只有 description 的候选
  当 validate_envelope 运行
  那么 返回诊断指名 label 字段

场景: 验证信封带重放上下文
  测试: test_verification_envelope_records_replay_context
  假设 一次以 stub 模式并指定变更路径发出的验证提问
  当 读取该信封的重放上下文
  那么 ai_mode 为 stub 且变更路径与发出时一致

场景: 证据归一化掉构建噪音
  测试: test_verification_questions_normalize_cargo_rerun_noise
  假设 同一场景在冷编译与热编译下各产出一次测试输出
  当 两次分别构建验证问题
  那么 两次携带的证据相同
