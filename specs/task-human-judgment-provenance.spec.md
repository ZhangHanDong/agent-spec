spec: task
name: "Human Judgment Provenance"
tags: [provenance, verification, audit]
satisfies: [REQ-HUMAN-JUDGMENT-PROVENANCE]
depends: [task-decision-point-emission]
risk: A
---

## Intent

把人工判定升为一等证据：写进 run log 与 trace 证据链，让 replay 与
explain-failure 能显示某条通过是否依赖人判。同时守住 ADR-001 的身份禁令
——记录判定的类别与内容、绑定证据 digest，绝不记录审批者身份，并用机械
检查防止身份字段随实现演进渗入。

## Decisions

- 判定记录形状：`{ source: "human", verdict, reasoning, scenario_id,
  evidence_digest }`。`source` 是类别枚举（human | model），不是身份。
- `evidence_digest` 复用既有 run log 的 digest 算法，外部系统按它在自己的
  存储里绑定审批人；本仓库不存任何绑定关系。
- 被禁字段检查为独立函数，对判定记录的序列化结果做键名扫描，命中
  `actor`/`authority`/`approval`/`policy` 即失败并指名该键。
- `requirements replay` 与 `requirements explain-failure` 在输出中以固定
  标记显示该通过依赖人工判定；无人工判定时输出逐字节不变。

## Boundaries

### Allowed Changes
- src/spec_knowledge/trace_ledger.rs
- src/spec_knowledge/questions.rs
- src/spec_core/verify.rs
- src/main.rs
- fixtures/**
- specs/task-human-judgment-provenance.spec.md

### Forbidden
- 不引入 actor、authority、approval 或 policy 字段
- 不以别名字段承载审批者身份
- 不改变无人工判定时的既有输出

## Out of Scope

- 外部审批存储与绑定协议
- 判定的多人会签或仲裁
- 发射点本身（emission 合约负责）

## Completion Criteria

场景: 人工判定进入证据链
  测试: test_human_judgment_enters_run_log
  假设 一个 uncertain 场景被人判定为通过
  当 该判定合入验证报告并写入 run log
  那么 记录含 verdict、reasoning 与该场景的证据 digest

场景: 判定按类别而非身份记录
  测试: test_judgment_records_class_not_identity
  假设 一条人工判定记录
  当 读取该记录字段
  那么 记录 source 为 human 且不含审批者姓名或标识

场景: 被禁字段被机械拦截
  测试: test_forbidden_identity_field_is_rejected
  假设 一条被注入 actor 字段的判定记录
  当 被禁字段检查运行
  那么 检查失败且错误信息指名 actor 字段

场景: 四个被禁字段逐个被拦
  测试: test_all_forbidden_fields_are_rejected
  假设 分别注入 actor、authority、approval 与 policy 的四条记录
  当 被禁字段检查逐条运行
  那么 四次均失败且各自指名对应字段名

场景: replay 显示判定依赖
  测试: test_replay_shows_human_judgment_dependency
  假设 一份含人工判定通过的历史运行
  当 requirements replay 针对该需求 id 运行
  那么 输出标明该通过依赖人工判定

场景: explain-failure 显示判定依赖
  测试: test_explain_failure_shows_human_judgment
  假设 一份含人工判定的失败运行
  当 requirements explain-failure 运行
  那么 输出标明哪些场景依赖人工判定

场景: 无人工判定时输出不变
  测试: test_machine_only_run_output_unchanged
  假设 一份全部由机器判定的验证运行
  当 run log 写入并被 replay 读取
  那么 输出不含任何人工判定标记
