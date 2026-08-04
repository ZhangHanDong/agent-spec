spec: task
name: "Decision Point Emission"
tags: [knowledge, questions, cli, verification]
satisfies: [REQ-DECISION-POINT-EMISSION]
depends: [task-decision-point-envelope]
risk: A
---

## Intent

把决策点信封从三个阶段真正发出来：`requirements questions` 输出带阶段标记
的信封并接受 agent 回填的候选，新增 `knowledge questions <id>` 从提案未决
问题与决策备选提取选型，新增 `verify --emit-questions` 把机器判不了的场景
变成待判定问题——后者发出的正是 `resolve-ai` 既有的输入形状。

## Decisions

- `requirements questions` 复用既有输出路径，仅补 kind 与
  `envelope_version`；候选回填走 `--options <file>` 读 agent 起草的 JSON，
  读入后跑 `validate_envelope`，诊断非空则退出码 2。
- `knowledge questions <id>` 解析 proposal 的 `## Unresolved Questions`
  列表项与 decision 的 `## Alternatives Considered` 列表项，每个列表项一条
  问题；推荐标记只在源文本含「recommended」或「推荐」时置位。
- `verify --emit-questions` 为 verdict 为 uncertain 或 pending_review 的
  场景各产一条信封，候选固定为 verdict 词汇表（pass / fail / skip），
  证据取自该场景已收集的 evidence 字段。
- 判定答案到 decisions JSON 的转换在 `verify --emit-questions` 的输出形状
  上直接对齐 `AiDecision` 字段名，避免二次映射。
- 三个发射点均不读标准输入；无问题时输出空集合、退出码 0。

## Boundaries

### Allowed Changes
- src/main.rs
- src/spec_knowledge/questions.rs
- src/spec_knowledge/mod.rs
- src/spec_verify/**
- fixtures/**
- specs/task-decision-point-emission.spec.md

### Forbidden
- 不在 CLI 内生成候选内容
- 不发起交互式提问或等待标准输入
- 不改动 resolve-ai 既有输入格式

## Out of Scope

- 判定进 run log 与 trace（provenance 合约负责）
- 回填文档的自动改写
- 统一 questions 命名空间

## Completion Criteria

场景: 逆向访谈问题带阶段标记
  测试: test_requirements_questions_emit_requirements_kind
  假设 一份含 compound-clause 诊断的需求文档
  当 requirements questions 以 json 输出
  那么 对应信封 kind 为 requirements 且 diagnostic_code 保留原诊断码

场景: 回填候选被校验
  测试: test_requirements_questions_validates_supplied_options
  假设 一份含五项候选的 agent 起草文件
  当 requirements questions --options 读入该文件
  那么 退出码为 2 且诊断指名候选数上限

场景: 提案未决问题被提取
  测试: test_knowledge_questions_extracts_unresolved_questions
  假设 一份未决问题含两条的提案
  当 knowledge questions 针对该提案 id 运行
  那么 输出两个 kind 为 knowledge 的信封且 source 指向该提案路径

场景: 未明示推荐时不标记推荐
  测试: test_knowledge_questions_omits_unstated_recommendation
  假设 一份备选中未写明推荐项的决策
  当 knowledge questions 针对该决策 id 运行
  那么 输出候选均不带推荐标记

场景: 待判定场景带证据发出
  测试: test_verify_emit_questions_carries_scenario_and_evidence
  假设 一份验证报告含一个 uncertain 场景
  当 verify --emit-questions 运行
  那么 输出信封 kind 为 verification 且含该场景文本与已收集证据

场景: 判定答案可直接喂给 resolve-ai
  测试: test_emitted_judgment_answers_feed_resolve_ai
  假设 一组已回答的验收判定信封
  当 转换为 decisions JSON 并交给 resolve-ai
  那么 resolve-ai 读取成功且无需改写字段名

场景: 无待决问题时输出空集合
  测试: test_emission_points_emit_empty_set_without_questions
  假设 一个无任何待决问题的工作区
  当 三个发射点分别运行
  那么 各自输出空集合且退出码为零

场景: 发射点不读标准输入
  测试: test_emission_points_never_read_stdin
  假设 标准输入被关闭的执行环境
  当 三个发射点分别运行
  那么 命令正常结束且不阻塞

场景: 提案无未决问题时不产问题
  测试: test_knowledge_questions_empty_for_resolved_proposal
  假设 一份未决问题写为 None 的提案
  当 knowledge questions 针对该提案 id 运行
  那么 输出空集合且退出码为零

场景: 未知 id 报可自愈错误
  测试: test_knowledge_questions_unknown_id_names_corpus
  假设 一个语料中不存在的知识 id
  当 knowledge questions 针对该 id 运行
  那么 退出码为 2 且错误信息指名该 id 与被扫描的知识根
