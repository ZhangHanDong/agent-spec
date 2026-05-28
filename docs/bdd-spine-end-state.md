# agent-spec: BDD-spine 七期完成后的使用效果

> 本文档描绘的是 `docs/comparison-openspec-speckit.md` §13 路线图全部落地后的最终使用状态。
> 当前进度: Phase 1 已写成可执行合约(`specs/task-bdd-semantics-v1.spec.md`), Phase 2–7 待实施。
> 建议阅读顺序: 先读本文档建立**目标感**, 再读 comparison 文档理解**为什么这样选**, 最后看 v1 合约知道**第一步怎么走**。

---

## 一句话总览

**agent-spec 成为唯一一个把 BDD 三层(Discovery → Formulation → Automation)完整闭环、且最后一层是机器跑出来的 spec-driven SDLC。** OpenSpec 的活规格库与 Spec Kit 的分发治理被 **Rule** 这一基元吸纳进同一份聚合契约里; 三层都用同一种 DSL 表达; 门禁不是看文档, 而是看机器跑出来的 verdict。

---

## 一个具体场景:Alice 给 agent-spec 加 `import speckit` 命令

下面五幕展示 Alice 整个工作流——**只用 `agent-spec` 一个 CLI**, 中间不需要 OpenSpec 也不需要 Spec Kit。

### 第一幕:Discovery —— 模糊想法 → 结构化未决问题

Alice 在 Claude Code 里说一句话:

> "想加个 `agent-spec import speckit <feature-dir>` 命令"

Agent 调用:

```bash
agent-spec init --level task --template import-flow \
  --capability ecosystem-import \
  --name "import-speckit-command"
```

生成的 `specs/task-import-speckit-command.spec.md` 已经包含 `## Questions` 顶层节(Phase 4 引入):

```spec
## Questions
- <!-- NEEDS CLARIFICATION: Spec Kit feature spec 的输入是单个 spec.md 还是整个 features/<name>/ 目录? -->
- <!-- NEEDS CLARIFICATION: 翻译后的 Test: selector 怎么处理? 留空还是写 NEEDS CLARIFICATION? -->
- <!-- NEEDS CLARIFICATION: 是否保留 Spec Kit 的 FR-001 编号作为 tag? -->
```

Alice 跑 `agent-spec lint --strict`:lint 检测到 3 个未决 `NEEDS CLARIFICATION`, **直接 fail**, 提示"Discovery 未完成, 先在 Agent 会话里解决问题"。Alice 和 Claude Code 对话回答完三个问题, 标记移除, lint 通过 Discovery 门禁。

**这一步替代了什么**:Spec Kit 的 `/clarify` 循环。但 agent-spec 不做对话引擎——对话发生在 Agent IDE 里, agent-spec 只校验产物形状 + 阻塞未决问题。

### 第二幕:Formulation —— Rule 引用 + 新 Example

Alice 让 agent 填 Decisions / Boundaries / Completion Criteria。关键差异在 `## 完成条件`:

```spec
## 完成条件

### Rule: speckit-fr-maps-to-scenario — 每条 FR 转成一个未绑定 Scenario
  Example: 单个 FR 生成 Scenario 草案
    测试:
      过滤: test_import_speckit_fr_to_scenario_draft
    假设 输入是含 `FR-001 系统必须支持 OAuth` 的 spec.md
    当 运行 `agent-spec import speckit features/oauth/spec.md`
    那么 输出 task contract 包含一个 `Scenario:` 草案
    并且 该 scenario 的 `Test:` selector 为空, 并标 `<!-- NEEDS CLARIFICATION -->`

  Example: 多条 FR 各自独立
    ...

### Rule: capability:ecosystem-import.import-must-preserve-traceability  ← 引用 capability scope
  Example: 翻译后保留来源 ID
    ...
```

第二个 Rule **引用 `specs/capabilities/ecosystem-import.spec.md` 已有的 Rule**(假设之前已为 ecosystem-import 这个 capability 立了几条根 Rule)。Alice 不重复定义, 只 instantiate Example。

`agent-spec contract` 渲染输出:

```
Rule: speckit-fr-maps-to-scenario (task:import-speckit-command)
    每条 FR 转成一个未绑定 Scenario
  ├─ Example: 单个 FR 生成 Scenario 草案    [test: test_import_speckit_fr_to_scenario_draft]
  └─ Example: 多条 FR 各自独立              [test: test_import_speckit_handles_multi_fr]

Rule: import-must-preserve-traceability (capability:ecosystem-import) ← inherited
    [Capability spec 提供根定义]
  └─ Example: 翻译后保留来源 ID            [test: test_import_speckit_preserves_fr_ids]
```

**这一步替代了什么**:OpenSpec 的 `## ADDED Requirements / ## MODIFIED Requirements` delta;Spec Kit 的 `FR-001` 编号 + Acceptance Scenarios 分离。在 agent-spec 里都是**同一个 Rule 基元在不同 scope 上的实例化**, 没有四种 delta operation 各自的格式仪式, 没有 proposal/design/tasks 拆文件。

### 第三幕:Automation —— 一条命令打通 lint + 测试执行 + 覆盖矩阵

Alice 让 agent 实现代码, 然后:

```bash
agent-spec lifecycle specs/task-import-speckit-command.spec.md \
  --code . --format markdown --change-scope worktree
```

输出(节选,**Phase 2 的覆盖矩阵 + Phase 5 的五维 lint 都在这里**):

```markdown
# Contract Acceptance: import-speckit-command

## Coverage Matrix
| Rule                                       | Scope                          | Example                          | Test selector                            | Test ✓ | Verdict | Boundary |
|--------------------------------------------|--------------------------------|----------------------------------|------------------------------------------|--------|---------|----------|
| speckit-fr-maps-to-scenario                | task                           | 单个 FR 生成 Scenario 草案       | test_import_speckit_fr_to_scenario_draft | found  | pass    | ✓        |
| speckit-fr-maps-to-scenario                | task                           | 多条 FR 各自独立                 | test_import_speckit_handles_multi_fr     | found  | pass    | ✓        |
| import-must-preserve-traceability          | capability:ecosystem-import    | 翻译后保留来源 ID                 | test_import_speckit_preserves_fr_ids     | found  | pass    | ✓        |

## Quality Report (5-dim)
- Completeness:  100%  ✓
- Clarity:        92%  (1 warning: scenario step uses "适当", 建议明确)
- Consistency:   100%  ✓
- Coverage:      100%  ✓ (所有 Rule 都有 Example, 所有 Example 都有绑定测试且测试存在)
- Boundary:      100%  ✓ (5 个改动文件全部落在 Allowed Changes)

## Verdict: PASS (3/3 scenarios passing, 0 skipped, 0 uncertain)
```

`is_passing` 公式不动 —— 这是机器证明的**当下事实**:3 条 Example, 每条都对应一个真实存在的 Rust 测试函数, 跑过、变绿、边界不越界。

**这一步替代了什么**:Spec Kit `/analyze` 的 LLM 推断覆盖表(现在是机械覆盖矩阵, 严格更准);OpenSpec 的 task checkbox(现在是测试 verdict);Spec Kit `/checklist`(现在是五维 quality 报告)。

### 第四幕:Promotion —— 任务级 Rule 沉淀到 capability 真相库

`Rule: speckit-fr-maps-to-scenario` 在这个任务里被两个 Example 反复证明了。Alice 觉得它是 import 能力的稳定不变量, 值得提到 capability scope:

```bash
agent-spec promote specs/task-import-speckit-command.spec.md \
  --rule speckit-fr-maps-to-scenario \
  --to capability:ecosystem-import
```

agent-spec 做两件事:

1. **前置校验**:此 Rule 在 task 里所有 Example verdict 为 pass;`is_passing` 通过;lint 无 critical。
2. **合并**:把 Rule 追加到 `specs/capabilities/ecosystem-import.spec.md`,id 不变, scope 字段从 `Task("import-speckit-command")` 变为 `Capability("ecosystem-import")`, Example 引用留在 task spec(作为历史证据)。

下一次有人写 `agent-spec import openspec` 任务时, 同一 Rule 自动出现在继承链里, 无需重新定义。

**这一步替代了什么**:OpenSpec 的 `archive` + delta merge —— agent-spec 这里更简洁, 因为 `promote` **前置门禁是真实跑过的测试**, 而不是文档完整性。

### 第五幕:PR & 长期治理 —— stamp + project.spec hard fail

```bash
agent-spec explain specs/task-import-speckit-command.spec.md --code . --format markdown > PR_BODY.md
agent-spec stamp specs/task-import-speckit-command.spec.md --code . --dry-run
```

输出的 PR 描述自动包含:覆盖矩阵 + 五维质量分 + 提升的 capability Rule + 通过的 verdict + git trailers (`Spec-Name:`, `Spec-Passing:`, `Spec-Rule-Promoted:`)。

Reviewer 不再看代码 diff —— 只检查两件事:

1. **Contract 定义对吗?**(Intent / Rule 写得对)
2. **机器有没有都通过?**(覆盖矩阵全绿、boundary 没越界、project.spec 无 critical 冲突)

如果 Alice 不小心改了 `src/spec_verify/test_verifier.rs`, 而 task 没声明这个 capability / 没继承允许此变更的 Rule, **Phase 5 升级后的 project.spec 冲突恒为 critical** —— guard hard fail, PR 进不来。

**这一步替代了什么**:Spec Kit 的 `taskstoissues` 与 `constitution.md` governance —— 在 agent-spec 这里, 治理是 `project.spec.md` 的 Rules 直接被 lint/guard 机械检查, 违反 = critical = block。

---

## 看不见的两个好处

### Phase 6:单源多工具生成 —— 永远不漂移

Alice 不需要分别维护 `AGENTS.md` / `.cursorrules` / `.claude/skills/`。一份源, 跑一次:

```bash
agent-spec install --target all
```

所有 Agent 工具同步刷新到当前 task contract 的指令集。Codex / Claude Code / Cursor / Aider 看到的都是同一份契约描述, 没有"哪个文件忘了改"。

### Phase 7:跨语言 runner —— NFR 不再装 pass

Task spec 里写:

```spec
### Rule: import-must-be-fast — 大型 Spec Kit feature 翻译 < 2s
  Example: 100 个 FR 的 feature 在 2s 内翻译完
    测试:
      Runner: criterion
      过滤: bench_import_speckit_100_frs
      阈值: p95 < 2000ms
```

`lifecycle` 调用 criterion benchmark runner(而不是 cargo test), 把 p95 时延 vs 阈值变成机械 pass/fail。p99 长尾 / 1000 并发 / 90% 成功率这类 NFR 都能用对应 runner 或外部探针(curl health check、Prometheus 查询、负载工具)接入, verdict 通道保持一致 (`pass` / `fail` / `uncertain`)。

---

## 不同角色的"日常感受"

### 对开发者(Alice)

写一份 `.spec.md`, 跑一条 `lifecycle`, 要么全绿, 要么具体到"哪个 Rule 下的哪个 Example 失败 / 测试不存在 / 边界越界"。不需要在 OpenSpec 里建 change folder、补 proposal+design+tasks 四个文件、archive 时再校验一次;也不需要在 Spec Kit 里走 7 个阶段命令、构造 constitution、跑 analyze 让 LLM 判一致性。**一份契约, 一条命令, 机器证明。**

### 对 Agent(Claude Code / Codex)

`agent-spec plan --format prompt` 输出一份按 Rule 分组的自包含 prompt, agent 看着 Rule + Boundary + Example + 现有代码上下文写实现。卡住时输出 `uncertain`, 触发 caller mode, 把 AI 判断变成结构化、可审计的证据, 但 verdict 永不默认 pass。

### 对 Reviewer / PR

`agent-spec explain` 一段 Markdown 就是 PR 描述。审查从"逐行 diff"变成"Contract 定义对吗 + 矩阵全绿吗"两个问题。

### 对项目长期治理

- `specs/capabilities/` 是系统当前行为的真相库(Phase 3)。
- `specs/project.spec.md` 是不可让步的项目宪法(Phase 5)。
- `specs/task-*.spec.md` 是每次变更的事实记录。

三层都是同一种 Rule 基元, 继承链由文件系统 + RuleScope 机械保证。

---

## 不假装解决的三件事

1. **完全的冷启动空白**:如果项目没有任何代码、没有测试基础, agent-spec 的价值仍然不大——它需要至少一个可被绑定的测试运行器或外部探针。Discovery 层(Phase 4)可以帮你写 Rules 与 Questions, 但不会替你想清楚业务。
2. **`Test:` selector 重命名风险**:覆盖矩阵能机械检出 dangling selector, 但改测试名仍然是手动维护成本。这是机械护城河的对价。
3. **非可测的主观质量**:即便 Phase 7 的 NFR runner 能测时延 / 并发 / 成功率, "用户觉得好用吗 / 代码可读性 / 架构是否优雅"这类需要人类判断的指标, agent-spec 只能存进 `pending_review` 通道, 不能机械给 pass。

---

## 与 OpenSpec / Spec Kit 的关系

agent-spec 自身完整, **不需要**它们。但提供**单向迁移坡道**(详见 comparison 文档 §11 Phase 5+ 可选: 迁移工具)让现有用户体面切换:

- `agent-spec import speckit <feature-dir>` —— 一次性把 Spec Kit feature 翻译成 task contract 草案。
- `agent-spec import openspec <change-dir>` —— 同上, 来源是 OpenSpec change 目录。
- `agent-spec export openspec-verification <task-spec>` —— 把 lifecycle 结果写成 OpenSpec verification.json。
- `lifecycle --capability-source-readonly <openspec-specs-dir>` —— 高级渐进迁移模式, 临时方案。

**长期共存不在推荐范围内**, 与吸纳叙事冲突——会同时承担多套工具的维护负担, 抵消"一份合约, 三层闭环"的核心承诺。

---

## 相关文档

- 战略叙事 + 吸纳路线: [`docs/comparison-openspec-speckit.md`](comparison-openspec-speckit.md)
- Phase 1 可执行合约: [`specs/task-bdd-semantics-v1.spec.md`](../specs/task-bdd-semantics-v1.spec.md)
- 项目级 Rules: [`specs/project.spec.md`](../specs/project.spec.md)

---

要从这个愿景里挑一件最先建出来摸的——Phase 1 的合约现在就在 `specs/task-bdd-semantics-v1.spec.md` 等着实施。
