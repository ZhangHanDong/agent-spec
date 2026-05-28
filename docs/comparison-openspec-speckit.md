# agent-spec vs OpenSpec vs Spec Kit: 三方深度对比

> 调研对象:
>
> - `agent-spec` 本仓库, Rust, `agent-spec` CLI
> - `OpenSpec` (`~/Work/Projects/FW/rust-agents/openspec`), TypeScript, `openspec` CLI
> - `Spec Kit` (`~/Work/Projects/FW/rust-agents/spec-kit`), Python, `specify` CLI
>
> 目的: 明确三者在 spec-driven development 链路中的不同位置, 找出 agent-spec 的真实差异化, 并提炼 OpenSpec / Spec Kit 中值得 agent-spec 借鉴的机制。

---

## 0. 结论先行

**BDD 三层闭环(Discovery → Formulation → Automation)是比 OpenSpec / Spec Kit 都更基础的统一框架。agent-spec 以 BDD 为脊柱, 在一个聚合的 Task Contract 里同时承担三层, 吸纳两者最重要的能力但不复制它们的目录与流水线。**

三者在 BDD 三层上的覆盖差异:

```text
              Discovery                              Formulation                         Automation
Spec Kit      /clarify, /specify, constitution       spec/plan/tasks                     —
OpenSpec      proposal.md 的 Why                     specs + delta + archive             —
agent-spec    结构化产物 + lint(Phase 4)             Task Contract + Rule→Example(v1)   Test selector + lifecycle + guard (唯一拥有)
```

更具体地说:

- **Spec Kit 把 Discovery 与 Formulation 做成命令模板 + 集成分发器。** 强项是把同一套 SDD 命令可靠投递到 30+ AI 工具运行时, 但完成信号停在文档 / checkbox。
- **OpenSpec 把 Formulation 做成长期规格库 + delta 归档。** 强项是 brownfield 变更管理(specs 是真相, changes 是 delta, archive 合并), 但归档前不机械验证代码行为。
- **agent-spec 把 Automation 做到底, 并以同一个 BDD 模型上溯 Formulation 与 Discovery。** Task Contract 中的 scenario 显式绑定测试 selector; lint、boundary verifier、test verifier 构成机械验证核心; AI verifier 作为结构化、可审计的补充层。

核心判断:

1. **BDD 三层是统一脊柱, 不需要分裂成三个工具。** agent-spec 用单一聚合的 Task Contract(加上 Rule → Example 基元)就能承担三层, 比 OpenSpec 的 4-artifact 拆分或 Spec Kit 的 7-phase 流水线都紧凑。
2. **Rule 是可提升的同一种基元, 在 task / capability / project 三个 scope 都成立。** 这是把 Spec Kit 的 constitution(项目级 Rule)与 OpenSpec 的 capability spec(能力级 Rule)用同一个 DSL 概念收掉的架构关键。详见 §9.3。
3. **机器执行测试 + 边界强制是不可让步的底线。** 借鉴上游能力可以扩展 agent-spec 的形态, 但 `is_passing` 的硬门禁与 `skip != pass` 永远不能软化。
4. **吸纳路线有明确适用边界。** agent-spec 仍偏下游验证, 价值曲线起步晚; 冷启动规划、NFR 验证依赖 Phase 4 的 Discovery 产物、Phase 7 的跨语言 runner / 外部探针, 或结构化 AI / caller 证据补齐。详见 §9.4。

---

## 1. 当前源码快照

| 项目 | 形态 | 当前版本 / 规模 | 关键入口 |
|---|---|---:|---|
| agent-spec | Rust CLI, 单一二进制 | `0.2.7`; 26 个 Rust 源文件; 52 个 `.spec` / `.spec.md`; 约 239 个 `#[test]` / `#[tokio::test]` 标记 | `Cargo.toml`, `src/main.rs`, `src/spec_gateway/lifecycle.rs` |
| OpenSpec | TypeScript npm CLI | `@fission-ai/openspec` `1.3.1`; 147 个 TS 源文件; 77 个 TS 测试文件; 29 个可选 AI tools; 26 个 command adapters | `package.json`, `src/cli/index.ts`, `src/core/init.ts`, `schemas/spec-driven/schema.yaml` |
| Spec Kit | Python package / Typer CLI | `specify-cli` `0.8.14.dev0`; 69 个 Python 源文件; 70 个 tests 下 Python 文件, 其中 63 个 `test_*.py`; 9 个核心命令模板; 30 个注册内置集成 | `pyproject.toml`, `src/specify_cli/__init__.py`, `src/specify_cli/integrations/` |

本文件基于本地源码静态审阅。未运行三方完整测试套件。

---

## 2. 一张总表

| 维度 | Spec Kit | OpenSpec | agent-spec |
|---|---|---|---|
| 核心问题 | 如何把 SDD 命令和模板装进不同 AI coding assistants | 如何维护长期规格库, 并以 change 为单位推进/归档变更 | 如何把任务契约变成机器可执行的验证门禁 |
| 主命令 | `specify init`; extension / preset / workflow / integration 子命令 | `openspec init`; `propose`, `apply`, `archive`, `validate`, `workspace` 等 | `contract`, `plan`, `lint`, `verify`, `lifecycle`, `guard`, `explain`, `stamp` |
| 核心 workflow | `specify -> review gate -> plan -> review gate -> tasks -> implement`; `clarify` / `checklist` / `analyze` 是可插入质量环节 | OPSX actions: `propose`, `explore`, `apply`, `sync`, `archive`; 可扩展到 `new`, `continue`, `ff`, `verify` 等 | Task Contract -> lint -> verify -> lifecycle/guard -> explain/stamp |
| 规格单元 | feature spec、plan、tasks、constitution | `openspec/specs/<capability>/spec.md` 和 `openspec/changes/<change>/...` | task spec / project spec / org spec |
| 长期真相层 | `constitution.md` + `.specify/` 项目配置; feature spec 偏一次性 | `openspec/specs/` 是系统当前行为真相 | `project.spec.md` 有继承治理, 但还没有 OpenSpec 式 capability spec 库 |
| 门禁性质 | 主要是 workflow gate 和文档一致性分析 | 主要是 artifact 完整性、delta spec 校验、task checkbox | 机械 lint + test execution + boundary verification + gate decision |
| 对代码的验证 | 没有直接跑代码验证实现是否满足每个 requirement | 没有直接跑代码验证实现是否满足每个 requirement | 有, 当前实现以 Rust/Cargo 为主, `TestVerifier` 执行 `cargo test` 选择器 |
| Agent 集成 | 集成类是单一事实源, 输出 Markdown / TOML / YAML / Skills 等 | `AI_TOOLS` + command adapter registry + skill/command generation | 自身提供 skills 和 AGENTS 指令, 核心是让 Agent 调 CLI |
| 验证失败语义 | 偏文档质量/一致性问题 | 偏 spec/delta/artifact 格式问题 | `pass`, `fail`, `skip`, `uncertain`, `pending_review` 明确区分 |
| 最强点 | 多 Agent 生态分发和 workflow engine | 活规格库 + delta archive + artifact graph | 契约到测试与边界的执行闭环 |
| 最大风险 | 流程和集成机器变重, 容易把严谨性寄托给 LLM 分析 | 轻量和灵活带来实现验证缺口 | 若补太多前端流程, 会稀释"验证器"定位 |

---

## 3. 三者的真实工作模型

### 3.1 Spec Kit: SDD 脚手架和 Agent 命令分发层

Spec Kit 的核心入口是 `specify init`。初始化阶段做的不是单纯生成一个 spec 文件, 而是安装整套项目脚手架:

- 选择或解析 AI integration。
- 生成 `.specify/` 下的模板、脚本、workflow 和 integration 状态。
- 安装核心命令模板, 例如 `analyze.md`, `checklist.md`, `clarify.md`, `constitution.md`, `implement.md`, `plan.md`, `specify.md`, `tasks.md`, `taskstoissues.md`。
- 根据 Agent 类型把模板转换为不同文件格式和不同目录布局。
- 可选安装 preset、extension、workflow。

它的集成系统是 Python 类驱动的 registry。每个 integration 是 `src/specify_cli/integrations/<key>/` 下的自包含子包, 由 `INTEGRATION_REGISTRY` 注册。`IntegrationBase` 要求集成声明:

- `key`
- `config`
- `registrar_config`
- `context_file`

然后用 `MarkdownIntegration`, `TomlIntegration`, `YamlIntegration`, `SkillsIntegration` 等基类处理不同输出格式。

这意味着 Spec Kit 的重点是**把同一套 SDD 命令可靠地投递到不同 Agent 运行时**。它最像一个"SDD distribution layer"。

Spec Kit 的 workflow engine 也说明了这个定位: 默认 `speckit` workflow 不是从代码状态推导真伪, 而是按步骤调用安装好的 Agent 命令, 中间穿插 review gate。默认链路是:

```text
speckit.specify
  -> review-spec gate
  -> speckit.plan
  -> review-plan gate
  -> speckit.tasks
  -> speckit.implement
```

这很适合把团队拉进统一 SDD 流程, 但它的"硬"主要体现在安装、manifest、安全路径、集成一致性和 workflow 状态上, 不体现在实现代码的行为验证上。

### 3.2 OpenSpec: 活规格库和变更归档层

OpenSpec 的文档明确强调:

```text
fluid not rigid
iterative not waterfall
easy not complex
brownfield-first
```

它的核心模型是两个目录:

```text
openspec/
  specs/       # 当前系统行为的 source of truth
  changes/     # 每个待做变更一个目录
```

一次变更通常产生:

```text
openspec/changes/<change>/
  proposal.md
  design.md
  tasks.md
  specs/<capability>/spec.md
```

其中 `changes/<change>/specs/...` 不是完整系统规格, 而是 delta spec。默认 `spec-driven` schema 要求 delta operation 使用:

- `## ADDED Requirements`
- `## MODIFIED Requirements`
- `## REMOVED Requirements`
- `## RENAMED Requirements`

每个 requirement 使用 `### Requirement: <name>`, scenario 使用 `#### Scenario: <name>`, 并要求 normative wording 包含 `SHALL` 或 `MUST`。

OpenSpec 的核心价值不在"一次性生成 spec", 而在**让规格随项目演进**:

1. `propose` 创建 change。
2. `specs/design/tasks` 补齐 artifact。
3. `apply` 指导实现。
4. `archive` 校验 delta spec 并合并到 `openspec/specs/`。

它还把 workflow 做成数据: `schema.yaml` 定义 artifact、生成路径、依赖关系和 instruction。`ArtifactGraph` 可计算 build order、next artifacts、blocked artifacts、complete status。

这让 OpenSpec 很适合 brownfield 项目: 不只是"下一次任务怎么写", 而是"当前系统行为的规格库如何持续更新"。

### 3.3 agent-spec: 契约执行和验证门禁层

agent-spec 的 README 把核心说得很清楚:

```text
humans review the contract
agents implement against the contract
the machine verifies whether the code satisfies the contract
```

它的主要对象不是 proposal/design/tasks 四个 artifact, 而是一个 Task Contract:

- `Intent`
- `Decisions`
- `Boundaries`
- `Completion Criteria`

Completion Criteria 里的每个 scenario 应显式绑定测试:

```spec
Scenario: Duplicate email is rejected
  Test: test_register_api_rejects_duplicate_email
```

或者结构化绑定:

```spec
Test:
  Package: user-service
  Filter: test_register_api_rejects_duplicate_email
```

这使 agent-spec 和另外两者产生根本差异: scenario 不只是文档单元, 还是验证入口。当前实现里, `TestVerifier` 会解析 `Test:` selector, 构造 `cargo test -q <filter>` 命令并执行。未被任何 verifier 覆盖的 scenario 会被 `run_verification` 标记为 `skip`, 而 `SpecGateway::is_passing` 明确要求 failed/skipped/uncertain 都为 0。跨语言 test runner 是后续扩展点, 不是当前实现事实。

agent-spec 的验证层包括:

- `StructuralVerifier`: 对 MustNot/Forbidden 约束做低成本模式检查。
- `BoundariesVerifier`: 对显式 change set 和 Allowed/Forbidden path 做机械匹配。
- `TestVerifier`: 执行绑定测试。
- `AiVerifier`: CLI 暴露 `off` / `stub` / `caller` mode; external/custom backend 是 host-injected 库级扩展点。它产出结构化 AI decision, 是可审计补充层, 不是机械验证核心。
- `ComplexityVerifier`: 例如 line ratio 这样的质量约束。

这条链路把 spec 从"Agent 应该读的说明"提升成了"CI/guard 可以执行的契约"。

---

## 4. 严谨性住在哪里

三者最大的区别不是语法, 而是严谨性的落点。

| 层次 | Spec Kit | OpenSpec | agent-spec |
|---|---|---|---|
| 格式严谨性 | 高: integration/preset/extension/workflow manifest 较强 | 高: schema、Markdown parser、delta spec 校验较强 | 高: DSL parser、lint pipeline、继承解析 |
| 需求严谨性 | 中高: `clarify`, `checklist`, `analyze` 通过 Agent/LLM 辅助发现模糊点 | 中高: requirement/scenario/delta 格式有机械校验 | 高: lint 直接检查 vague verb、testability、coverage、determinism、test binding 等 |
| 实现严谨性 | 低: 不运行代码验证 requirement | 低: 不运行代码验证 requirement | 高: scenario 可绑定测试并实际执行 |
| 边界严谨性 | 中: context 和 workflow 能提示, 但不机械检查改动越界 | 低中: artifact 边界清楚, 代码边界不强制 | 高: Allowed/Forbidden paths 可对 change set 做机械验证 |
| 结果严谨性 | 偏过程完成 | 偏 artifact 完成和 archive 成功 | 偏验证结果: pass/fail/skip/uncertain |

这解释了 agent-spec 的位置: **它不是为了替代前端规划工具, 而是补齐前端工具没有覆盖的执行门禁。**

Spec Kit 的 `/analyze`、OpenSpec 的 `validate` 都有价值, 但它们验证的是文档结构、需求质量、artifact 状态。agent-spec 的 `lifecycle` 和 `guard` 验证的是:

- spec 是否足够清晰。
- scenario 是否有可执行绑定。
- 绑定测试是否通过。
- 未覆盖 scenario 是否阻塞。
- 变更路径是否越界。
- 质量门槛是否达标。

这是一个不同的严谨性层级。

### 4.1 时间维度: executable specs 更难悄悄过期

上表讲的是静态严谨性, 但三者更深的分水岭在时间维度。

OpenSpec 的 `openspec/specs/` 是 living specs: 它们是系统当前行为的真相, 但这个真相需要人和 Agent 持续维护。代码改了, 如果没有人更新 specs 或重新 validate 对应 change, 规格和实现可能静默漂移。

Spec Kit 的 feature spec / plan / tasks 也类似。它们很适合在实现前对齐意图, 但实现完成后, spec 与代码的一致性需要人、Agent 或外部 CI 重新建立。

agent-spec 的 Task Contract 更接近 executable specs: 每次 `verify`, `lifecycle`, `guard` 都会重新检查当前代码是否仍满足契约。一个 passing contract 不是说"当时写得对", 而是说"代码此刻仍能通过这份契约绑定的验证"。

这个结论有前提: `Test:` selector 仍指向有意义的测试, 测试本身没有退化成空断言。agent-spec 不能保证测试永远充分; 当前实现能暴露 failing、skipped、uncertain, 但 dangling selector 还需要后续覆盖矩阵或 test-found 检查来识别。这样才能让更多漂移从静默状态变成可见信号。

所以 OpenSpec 的规格是"活的", 靠人续命; agent-spec 的规格是"可执行的", 靠机器续证。这个差异比"有没有代码验证"更准确: agent-spec 的价值不只是验证一次, 而是让契约可以被反复验证。

---

## 5. 数据模型对比

### 5.1 Spec Kit 的数据模型: 命令模板 + 项目脚手架 + 集成 manifest

Spec Kit 的核心数据不是一个长期规格库, 而是一组可安装资产:

```text
templates/
  commands/
scripts/
extensions/
presets/
workflows/
src/specify_cli/core_pack/
```

初始化后, 它把资产复制到项目中, 并记录 integration manifest。manifest 记录文件哈希, 用于安全卸载和检测用户修改。这是典型的"分发器"设计。

Spec Kit 有 `constitution.md`, 但它更像项目原则和治理文本。它能影响 `/analyze` 的 severity, 但不是机械可执行的行为契约。

### 5.2 OpenSpec 的数据模型: specs + changes + schema

OpenSpec 的核心数据结构是:

```text
openspec/
  config.yaml
  specs/<capability>/spec.md
  changes/<change>/
    proposal.md
    design.md
    tasks.md
    specs/<capability>/spec.md
```

这个模型比 Spec Kit 更像"产品规格数据库"。最关键的是它区分:

- **当前真相**: `openspec/specs/`
- **待合并变更**: `openspec/changes/<change>/`

`archive` 是 OpenSpec 的关键动作: 把 change 里的 delta spec 应用到 main specs, 再把 change 移入 archive。

### 5.3 agent-spec 的数据模型: Task Contract + project/org inheritance

agent-spec 的核心 AST 包括:

- `SpecMeta`: level、name、inherits、lang、tags、depends、estimate。
- `Section`: Intent、Constraints、Decisions、Boundaries、AcceptanceCriteria、OutOfScope。
- `Scenario`: name、steps、test_selector、tags、review、mode、depends_on。
- `TestSelector`: filter、package、level、test_double、targets。

它支持三层继承:

```text
org.spec(.md) -> project.spec(.md) -> task.spec(.md)
```

当前本仓库已有 `specs/project.spec.md`, 用项目级 Must/Must Not 规则约束 task spec。例如:

- 验证结果必须区分 `pass`, `fail`, `skip`, `uncertain`。
- 不要把 `skip` 记为 `pass`。
- 任务级完成条件中的每个场景应显式声明测试 selector。
- guard / verify / lifecycle 应支持 change scope。

不过 agent-spec 目前还缺 OpenSpec 式的 **capability-level living specs**。它有项目级治理, 有 task-level contract, 但没有一个 `specs/<capability>/spec.md` 风格的"系统当前行为全集"。

另一个容易被低估的差异是双语 authoring。agent-spec 的 parser 同时识别中文和英文 section / BDD 关键字, 例如 `意图` / `Intent`, `完成条件` / `Completion Criteria`, `场景` / `Scenario`, `假设/当/那么/并且/但是` 与 `Given/When/Then/And/But`。`SpecMeta.lang` 也把语言作为 AST 字段保存。OpenSpec 和 Spec Kit 的主模板与 parser 主要是英文 Markdown 生态; 对中文团队来说, agent-spec 的中英双语 DSL 是实际产品壁垒, 不是翻译层面的文案差异。

---

## 6. 生命周期对比

### 6.1 Spec Kit 生命周期

```text
init
  -> constitution
  -> specify
  -> clarify
  -> plan
  -> tasks
  -> analyze/checklist
  -> implement
```

Spec Kit 的生命周期像一条默认 SDD 生产线。它适合 greenfield 或需要统一团队工作流的场景。默认 workflow 还内置 review gate, 强制人在关键节点确认。

风险是: 流水线越完整, 用户越容易以为"流程走完就是完成"。但 Spec Kit 并没有把 feature requirement 绑定到测试执行结果, 所以完成信号仍然要靠人/Agent/CI 的其他机制补齐。

### 6.2 OpenSpec 生命周期

```text
propose
  -> create/edit artifacts in changes/<change>
  -> apply
  -> validate
  -> archive into specs/
```

OpenSpec 不强调固定阶段, 强调 actions。它甚至明确反对 rigid phases。Artifact dependency 是 enabler, 不是强制 waterfall。

这比 Spec Kit 更贴近 brownfield 开发: 你可以先探索, 也可以先补 spec, 再补 design/tasks。最后通过 archive 把 change 合并到长期规格库。

风险是: apply/verify 更多是引导 Agent 做事, 不是严格证明实现正确。它能保证 spec delta 格式和 archive 过程, 但不能保证实现代码真的满足每个 scenario。

### 6.3 agent-spec 生命周期

```text
author task contract
  -> lint / quality gate
  -> contract / plan for Agent
  -> implement
  -> lifecycle: lint + verify + report
  -> guard: repo-level check
  -> explain / stamp
```

agent-spec 的 lifecycle 是门禁而不是流程剧本。Agent 可以怎么实现不重要, 关键是:

- spec 质量过关。
- scenario 被 verifier 覆盖。
- 绑定测试通过。
- change set 没越界。
- `skip` 和 `uncertain` 不被当成 pass。

这是三者中唯一把"完成"定义为机器可验证结果的工具。

---

## 7. Agent 集成对比

### 7.1 Spec Kit: IntegrationBase 体系

Spec Kit 的集成体系最成熟。它把每个 Agent 的差异封装成 integration class, 由 registry 统一导出。当前有 30 个注册内置 integration, 覆盖 Codex、Claude、Gemini、Copilot、Goose、Forge、Windsurf 等。

输出格式按基类分层:

- `MarkdownIntegration`
- `TomlIntegration`
- `YamlIntegration`
- `SkillsIntegration`
- 自定义 `IntegrationBase`

Codex 在 Spec Kit 中是 skills-based integration, 默认写入:

```text
.agents/skills/speckit-<name>/SKILL.md
```

这套设计的优点是强一致、可测试、可卸载。缺点是生态机器很重, 每加一个 Agent 都要维护注册、格式、安装、上下文文件和测试。

### 7.2 OpenSpec: AI_TOOLS + Adapter Registry

OpenSpec 的工具支持分两层:

- `AI_TOOLS`: 定义可选工具、skillsDir、检测路径。
- `CommandAdapterRegistry`: 定义可生成 command 文件的 adapter。

它支持 29 个可选工具, 但不是每个工具都有 command adapter。例如 docs 中明确说明 ForgeCode、Kimi、Trae 没有 command adapter, 走 skill-based invocation。

Codex 的 OpenSpec command 是特殊的: command 文件写到全局 `$CODEX_HOME/prompts/opsx-<id>.md`, 而不是项目目录。这对单人使用方便, 但对团队共享和 repo-local 可移植性不如项目内 commands/skills。

OpenSpec 默认 profile 是 `core`, 包含:

```text
propose, explore, apply, sync, archive
```

扩展 profile 可加入:

```text
new, continue, ff, verify, bulk-archive, onboard
```

### 7.3 agent-spec: Tool-first skills

agent-spec 不试图给每个 Agent 生成自己的命令文件。它的主路径是 tool-first:

```text
Agent reads AGENTS.md / skill
  -> calls agent-spec CLI
  -> interprets JSON/text/markdown output
```

这和 Spec Kit / OpenSpec 是反向关系:

- Spec Kit / OpenSpec 把自己的工作流"嵌入" Agent。
- agent-spec 让 Agent 调一个外部验证器。

这正是它保持小而硬的原因。Agent 集成不是产品核心, CLI 验证语义才是核心。

---

## 8. 机械验证对比

### 8.1 Spec Kit 的验证边界

Spec Kit 验证强项:

- integration manifest 记录安装文件哈希。
- extension/preset manifest 有 schema 和 required fields。
- workflow YAML 有 engine-side load/validate/execute/resume。
- command templates 会按 Agent 格式转换, 并做 path rewrite。

Spec Kit 对需求和实现的验证主要通过命令模板让 Agent 执行。例如 `/analyze` 和 `/checklist` 可以要求 Agent 检查不一致、模糊点、覆盖缺口。但这不是机械执行测试。

所以 Spec Kit 的验证边界是:

```text
脚手架/集成/流程状态: 机械
需求质量/一致性: Agent/LLM 辅助
代码行为满足需求: 不直接负责
```

### 8.2 OpenSpec 的验证边界

OpenSpec 验证强项:

- `Validator` 解析 main spec 和 change proposal。
- `validateChangeDeltaSpecs` 检查 delta spec:
  - 至少一个 delta。
  - ADDED/MODIFIED 要有 requirement text。
  - ADDED/MODIFIED 要包含 SHALL/MUST。
  - ADDED/MODIFIED 至少一个 scenario。
  - REMOVED / RENAMED 有各自结构要求。
  - 检查重复和跨 section 冲突。
- `ArchiveCommand` 在归档前校验 proposal 和 delta spec, 再把 delta 应用到 main specs。

OpenSpec 的验证边界是:

```text
规格格式 / delta 合法性 / archive 合并: 机械
任务 checkbox / artifact 完整性: 半机械
代码行为满足需求: 不直接负责
```

### 8.3 agent-spec 的验证边界

agent-spec 的验证边界更靠近代码:

```text
spec DSL / lint: 机械
scenario -> test selector: 机械
test selector -> cargo test: 执行
change set -> boundary globs: 机械
未覆盖 scenario -> skip: 阻塞
AI/caller -> structured supplemental decision: 可插拔 / 可审计
```

这使 agent-spec 能给出更强的完成语义:

```text
passing = total > 0
       && failed == 0
       && skipped == 0
       && uncertain == 0
       && (strict mode 下 pending_review == 0)
```

这条规则是 agent-spec 的根基。任何借鉴都不能削弱它。

### 8.4 Caller mode: 给 LLM 判断装护栏

agent-spec 不是简单拒绝 LLM 判断。它拒绝的是"LLM 读完文档后直接宣布完成"这种不可审计模式。

当前 `caller` mode 的协议是两步:

```text
agent-spec lifecycle <spec> --code . --ai-mode caller --format json
  -> 对未被机械 verifier 覆盖的 skipped scenarios 生成 AiRequest
  -> 写入 .agent-spec/pending-ai-requests.json

调用方 Agent 读取请求, 产出结构化 decisions JSON
  -> agent-spec resolve-ai <spec> --decisions decisions.json
  -> 合并为 VerificationReport
```

这套模式和 Spec Kit / OpenSpec 的文档层 LLM 分析不同。它把 AI 判断限制在明确的 scenario 请求里, 并保留:

- 请求文件: 可审计 AI 看到了什么。
- `verdict`: 不默认等同 pass。
- `confidence`: 置信度显式可见。
- `reasoning`: 决策理由进入证据链。
- 机械 verifier 优先: test/boundary/structural 已覆盖的结果不会被 AI 随意覆盖。

因此, agent-spec 的立场不是"不用 LLM", 而是"LLM 可以参与验证, 但必须被结构化、可追踪、可合并, 且不能替代机械证据"。

---

## 9. 产品定位: BDD-spine 吸纳

agent-spec 的定位不是与 OpenSpec / Spec Kit 互补的下游验证层, 而是**以 BDD 为脊柱、把它们最重要的能力吸纳进同一个聚合 Task Contract**。下面三节按这个口径重写。

### 9.1 OpenSpec 能力如何吸纳成 BDD-native 原语

| OpenSpec 能力 | agent-spec 中的 BDD-native 等价 | 落地 |
|---|---|---|
| `proposal.md` 的 Why / What Changes | task spec 的 `## 意图` + `## 已定决策`(同一聚合文件) | 已有 |
| `openspec/specs/<capability>/spec.md` 活规格 | `specs/capabilities/<name>.spec.md`, 主体内容是 Rules | Phase 3 |
| `### Requirement` / `#### Scenario` 嵌套 | capability spec 的 `Rule:` + task spec 的 `Scenario:` | v1 落 Rule, Phase 3 落 capability |
| `ADDED` / `MODIFIED` / `REMOVED` / `RENAMED` delta | task spec 声明 `capability: <name>` + 列新增 / 修改的 Rule(极简, 没有四种 operation 各自的格式仪式) | Phase 3 |
| `archive` 合并 delta | `agent-spec promote`: lifecycle 通过后把 task spec 的稳定 Rule 合并进 capability spec | Phase 3 |
| `ArtifactGraph` 就绪 / 阻塞查询 | scenario `depends_on` + spec `depends`(已有) + `plan --show-blockers` | 部分已有, 余下并入 Phase 2 |

OpenSpec 的 4-artifact 拆分(proposal/design/tasks 分文件)**不吸纳** —— agent-spec 的聚合 Task Contract 是上下文密度优势。OpenSpec 那么拆是为了在缺少验证层时维持跨文档一致性, agent-spec 不需要。

### 9.2 Spec Kit 能力如何吸纳成 BDD-native 原语

| Spec Kit 能力 | agent-spec 中的 BDD-native 等价 | 落地 |
|---|---|---|
| `constitution.md` 项目级宪法 | `project.spec.md` 中的 Rules(已支持继承); 违反恒为 critical severity | 已有继承, Phase 5 补 severity |
| `/specify` + `/clarify` + `[NEEDS CLARIFICATION]` | spec 内 `## Questions` 顶层节 + `<!-- NEEDS CLARIFICATION -->` 标记 + lint 阻塞 | Phase 4 (需同步改 `Section` enum / parser / lint / skills) |
| `/analyze` 覆盖一致性(LLM 推断) | **机械覆盖矩阵** Rule × Scenario × Test × Verdict —— 因有显式 `Test:` selector, 可机械填表, 严格优于 LLM 推断 | Phase 2 |
| `/checklist` 需求质量五维 | lint 报告按 Completeness / Clarity / Consistency / Coverage / Boundary 聚合 | Phase 5 |
| 30 integrations 分发 | 单一源 → `AGENTS.md` / `.cursorrules` / `.claude/skills/` 的轻量生成 | Phase 6, 替代手工维护的 `install-skills.sh` |
| `taskstoissues` 导 GitHub issue | 可作为 `agent-spec stamp` 的扩展输出 | 可选, 未排期 |

Spec Kit 的 **7-phase 刚性流水线**与 **extension / preset / workflow / catalog** 整套生态机器**不吸纳** —— agent-spec 是门禁不是流水线; 保持单一 Rust 二进制和 tool-first 集成路径。

### 9.3 Rule 作为可提升的同一基元(架构 keystone)

这是 BDD-spine 吸纳路线最核心的架构洞见。把 OpenSpec 的 capability 库与 Spec Kit 的 constitution 用**同一个 DSL 概念**统一掉:

| Scope | 写在哪 | 对应竞品概念 |
|---|---|---|
| `Project` | `specs/project.spec.md` 的 Rules | Spec Kit `constitution.md`(不可让步的项目级原则) |
| `Capability(name)` | `specs/capabilities/<name>.spec.md` 的 Rules | OpenSpec `openspec/specs/<capability>/spec.md`(系统当前行为真相) |
| `Task(name)` | task spec 的 `Rule:` 行 | 本次变更要实例化 / 新增 / 修改的规则 |

继承链: `project.Rules ⊆ capability.Rules ⊆ task 引用的 Rules`; Scenario / Example 始终住在 task spec, **证明**对应 scope 的 Rule。

身份模型(v1 锁定): `RuleKey = { scope, id }`。`id` 是稳定 kebab-case 标识符, `name` 是可任意修改的人类显示文本。提升一个 Rule 从 task → capability **只改 scope 字段, id / 引用全部保持稳定**, 不发生数据迁移。

v1(`specs/task-bdd-semantics-v1.spec.md`)落地 Task scope; `Capability` 与 `Project` scope 作为 `RuleScope` 枚举的 reserved 变体写入 AST, 但 v1 不解析、不加载、不提升 —— Phase 3 一次性完整上线。

### 9.4 agent-spec 的真实代价

为了获得机械验证闭环, agent-spec 付出了三个真实代价。把这些代价写清楚, 反而能让定位更可信。

第一, **价值曲线起步更晚**。Spec Kit 和 OpenSpec 在白纸规划阶段就有价值: 它们能帮助提出 spec、拆 plan、组织 change。agent-spec 要到有代码、测试或至少可验证边界时才产生主要价值。它单独解决不了冷启动规划问题, 所以更适合作为上游 spec 工具之后的验证层。

第二, **`Test:` selector 带来代码标识符耦合**。scenario 绑定到测试函数名以后, 测试改名会让契约出现 dangling selector。这是机械覆盖的对价: OpenSpec / Spec Kit 的规格与代码标识符更松耦合, 但也因此无法机械证明 scenario 被哪个测试覆盖。agent-spec 应通过覆盖矩阵缓解这个代价, 新增 test discovery 或解析 runner-specific no-test-detected signal, 例如 Cargo 的 `running 0 tests`, 作为 test-found 检查, 明确显示 selector 是否存在、是否匹配测试、最近 verdict 是什么。

第三, **非功能目标和可度量结果有覆盖天花板**。例如 "<2 分钟完成", "1000 并发", "90% 成功率" 这类 outcome 可以写成 contract, 但不总能塌缩成单个 `cargo test`。agent-spec 对可行为化、可测试的契约最强; 对性能、可靠性、可用性等 NFR, 需要专门 test runner、基准测试、外部探针或 AI/caller mode。诚实输出应是 `uncertain` 或 pending review, 而不是伪装成 pass。

---

## 10. agent-spec 应该借鉴什么

### P0: 必借, 直接增强核心差异化

#### 1. 机械覆盖矩阵

来源: Spec Kit `/analyze` 的 coverage report 思路。

Spec Kit 能让 Agent 生成类似 requirement coverage 的报告, 但 agent-spec 可以做得更硬, 因为它有显式 `Test:` selector。

建议输出:

```text
| Requirement / Constraint | Scenario | Test selector | Test found | Verdict | Boundary relevant |
|---|---|---|---|---|---|
```

这可以作为:

- `agent-spec lint --cross-check`
- `agent-spec explain --format markdown`
- `agent-spec lifecycle --format md`

的一部分。

关键是保持机械性:

- scenario 是否存在: parser 知道。
- Test selector 是否存在: AST 知道。
- selector 是否匹配测试函数: 需新增 test discovery, 或解析 runner-specific no-test-detected signal, 例如 Cargo 的 `running 0 tests`。
- verdict 是否 pass: VerificationReport 知道。

不要退化成 LLM 关键词猜测覆盖关系。

#### 2. 活规格库 / capability 层

来源: OpenSpec `openspec/specs/` 与 `openspec/changes/` 分离。

agent-spec 当前有:

```text
project.spec.md
task.spec.md
```

但缺:

```text
capabilities/<capability>.spec.md
```

或者:

```text
specs/capabilities/<capability>.spec.md
```

这层可以承载"系统当前行为", task contract 则承载"这次要改什么"。

建议模型:

```text
org.spec.md
  -> project.spec.md
    -> capability.spec.md
      -> task.spec.md
```

或保持三层不变, 但给 `project.spec.md` 增加 `capabilities/` 引用。不要照搬 OpenSpec 的四 artifact 拆分; 借的是"长期真相层", 不是 proposal/design/tasks 结构。

#### 3. Archive 前验证钩子

来源: OpenSpec archive 生命周期。

如果 agent-spec 做 OpenSpec integration, 最强钩子不是 `propose`, 而是:

```text
openspec archive <change>
  -> before applying delta:
       run agent-spec lifecycle/guard for mapped task contract
  -> only archive when verification passes
```

这会把 OpenSpec 的 archive 从"规格合并"提升成"经验证的规格合并"。

### P1: 应借, 能提升治理和 UX

#### 4. `project.spec` 冲突提升为最高 severity

来源: Spec Kit constitution governance。

Spec Kit 中 constitution 冲突应该是不可让步的项目级问题。agent-spec 已有 `project.spec.md`, 而且比 constitution 更适合机械校验。

建议:

- 违反 project-level Must/Must Not 的 lint 或 verification 结果标为 `critical`。
- `guard` 对 critical 一律 hard fail。
- `explain` 单独列出 Project Rule Violations。

这能把 `project.spec.md` 从"继承上下文"提升为"治理门禁"。

#### 5. `NEEDS CLARIFICATION` 去歧义循环

来源: Spec Kit `/clarify`。

Spec Kit 的好点子不是多一个命令, 而是在 spec 文本里留下明确 unresolved marker。

agent-spec 可以引入:

```text
<!-- NEEDS CLARIFICATION: scenario has no Test selector -->
<!-- NEEDS CLARIFICATION: boundary says "core files" but no path glob -->
```

建议:

- `lint --suggest-clarifications` 插入或输出 clarification markers。
- `lint` 默认把残留 marker 视作 warning 或 error。
- `lifecycle` 在 min-score 下自然阻塞。

这比抽象的 "human review loop" 更可执行。

#### 6. Artifact graph 的 ready/blocked UX

来源: OpenSpec `ArtifactGraph`。

agent-spec 已有 `depends` 和 scenario `depends_on`, 也有 `plan` 里的 task sketch。可以借 OpenSpec 的三类查询:

- `next`: 哪些 scenario/spec 已就绪。
- `blocked`: 哪些 scenario/spec 被哪些依赖卡住。
- `complete`: 是否全部完成。

用法:

```bash
agent-spec graph --spec-dir specs --format dot
# 可新增:
agent-spec plan specs/task.spec --show-blockers
```

注意: 借 graph 查询, 不借 OpenSpec 的多 artifact 拆分。agent-spec 的 Task Contract 仍应保持聚合式。

#### 7. Profile / template as data

来源: OpenSpec OPSX schema。

OpenSpec 把 workflow instructions 外置到 `schema.yaml` 和 templates, 避免改提示词就要发版。agent-spec 当前模板和部分 prompt 输出更偏 Rust 代码内置。

建议:

- `agent-spec init --template rewrite-parity` 继续保留。
- 增加 repo-local `.agent-spec/templates/*.spec.md` 覆盖点。
- 增加 profile:

```text
standard
rewrite-parity
api-contract
cli-parity
data-migration
```

重点是: profile 只影响 contract authoring 模板, 不影响 verification semantics。

### P2: 可借, 低成本改善质量

#### 8. OpenSpec 的解析陷阱规则

OpenSpec 对 scenario header、MODIFIED 完整复制等规则非常具体。agent-spec 可吸收为 lint 规则:

- scenario 标题层级/关键字不规范时提示。
- `MODIFIED` / rewrite parity 场景要求完整行为对照, 不允许只写 partial behavior。
- 当 spec 中出现 OpenSpec 风格片段时, 给出迁移提示。

#### 9. Spec Kit checklist 的五维报告

Spec Kit 的 checklist 思路适合 agent-spec lint UX:

- Completeness
- Clarity
- Consistency
- Coverage
- Boundary

agent-spec 已有多种 linter, 但报告可读性可以按这五类聚合。

#### 10. Agent-friendly degradation

来源: OpenSpec 对空项目、无 changes、无 tool 情况的友好提示。

agent-spec 的 CLI 是 Agent 会直接调用的工具, 所以每个错误都应回答:

- 发生了什么。
- 下一步命令是什么。
- 是否应该修改 spec 还是修改 code。

特别是:

- 无 spec 文件。
- spec 没有 Completion Criteria。
- scenario 没有 Test selector。
- `cargo test` 找不到 workspace。
- change scope 为空。

### P3: 明确不借

#### 1. 不借 Spec Kit 的完整生态机器

Spec Kit 的 integration/preset/extension/workflow 系统很强, 但不是 agent-spec 的核心。agent-spec 如果复制它, 会从验证器膨胀成分发平台。

agent-spec 应保持:

```text
small binary
tool-first
provider-agnostic
verification semantics first
```

#### 2. 不借 OpenSpec 的四 artifact 拆分

OpenSpec 拆 proposal/specs/design/tasks 是为了支持 change workflow 和 living specs。agent-spec 的优势是一个 Task Contract 同时包含 Intent、Decisions、Boundaries、Completion Criteria。

如果 agent-spec 把 task contract 拆成四个文件, 会损害 Agent 执行时的上下文密度。

#### 3. 不借"无门禁"哲学

OpenSpec 的 "fluid not rigid" 对规划阶段有价值, 但 agent-spec 的价值就是 gate。

agent-spec 可以允许工作方式灵活, 但 `lifecycle` / `guard` 的通过条件不能软化。

#### 4. 不借 LLM 判定实现完成

Spec Kit 和 OpenSpec 都会让 Agent/LLM 在文档层做分析。agent-spec 可以使用 AI verifier, 但必须保持:

- AI decision 结构化。
- AI confidence 可见。
- AI verdict 不默认等同 pass。
- caller mode 可审计; external/custom backend 是 host-injected 库级扩展点。
- 机械 verifier 优先。

---

## 11. 路线图建议

总图见 §13 的 Phase 1–7。本节给前两期的具体动作清单, 其余 phase 在 §13 概述。

### Phase 1: Formulation v1 — Rule → Example 基元

合约: [`specs/task-bdd-semantics-v1.spec.md`](../specs/task-bdd-semantics-v1.spec.md)。

具体动作:

1. DSL 增加显式 `Rule: <id>` / `规则: <id>` 头部, 强制 kebab-case id, 不做自动 slugify。
2. AST 增加 `BehaviorRule { key: { scope, id }, name, scenario_names }` 与 `RuleScope::{ Task(file_stem), Capability(name), Project }`; Capability / Project 写入但 v1 不解析。
3. `Example:` / `示例:` / `例子:` 作为 `Scenario:` 别名, 不引入新 AST 节点。
4. 新增三条 lint(全部 warning / info, 不入 verdict): `bdd-rule-id`、`bdd-rule-grouping`、`bdd-scenario-shape`、`bdd-implementation-detail-step`(中英文双语关键词)。
5. `contract` 与 `plan --format prompt` 按 Rule 分组渲染; 无 Rule 的旧 spec 保持当前扁平 `Scenario:` 列表格式。
6. 回归: `cargo test` 全集通过; `agent-spec guard --spec-dir specs --code .` verdict 与 v1 前**逐 spec 一致**(lint 输出可有新增非阻塞 info)。

### Phase 2: 机械覆盖矩阵 Rule × Scenario × Test × Verdict

目标: 把"显式 `Test:` selector 这个护城河"变现为一份 Phase 3 / 4 共同依赖的数据结构, 不需要新 DSL 改动。

具体动作:

1. `agent-spec lint --cross-check` 输出表:

   ```text
   | Rule | Scenario | Test selector | Test found | Verdict | Boundary relevant |
   |------|----------|---------------|------------|---------|-------------------|
   ```

2. `agent-spec explain --format markdown` 包含同一矩阵, 作为 PR-ready 验收材料。
3. `agent-spec lifecycle --format markdown` 输出同一矩阵 + summary 三段(Project Rule Violations / Boundary Violations / Test Binding Gaps)。
4. 机械性约束(不让 LLM 推断覆盖关系):
   - scenario 是否存在: parser 知道。
   - `Test:` selector 是否存在: AST 知道。
   - selector 是否匹配测试函数: code scan 或 `cargo test --list` 结果知道。
   - verdict: VerificationReport 知道。
5. 矩阵是 Phase 3 的 capability promote 与 Phase 4 的 Discovery 阻塞判断的共同数据底座。

### Phase 3-7

见 §13 闭合公式。其中:

- Phase 3 capability 层 + promote 是 OpenSpec 活规格库的 BDD-native 落地; task spec 通过 `capability: <name>` 引用 capability spec 的 Rules, lifecycle 通过后 `agent-spec promote` 把稳定 Rule 合并到 `specs/capabilities/<name>.spec.md`。
- Phase 4 Discovery 一次性改完 `Section` enum / parser / lint / skills / AGENTS, 引入 `## Questions` 顶层节与 `<!-- NEEDS CLARIFICATION -->` 标记; strict 模式 lifecycle 阻塞未决问题。
- Phase 5–7 见 §13 路线图。

### Phase 5+ 可选: 迁移工具(单向, 一次性)

agent-spec 走完 Phase 1–7 自身已完整, **不依赖** OpenSpec / Spec Kit。但为已经在它们上面积累 spec 的现有用户提供**单向迁移坡道**, 让上游用户能体面切换。这些命令明确**不是**"agent-spec 作为下游验证层永久共存"的接口:

1. `agent-spec import speckit <feature-dir>`:
   - 读取 Spec Kit feature spec / plan / tasks。
   - 抽取 FR / User Story / Acceptance Criteria → agent-spec `Scenario` 草案。
   - 抽取 `constitution.md` Must / Must Not → 写入或补充 `project.spec.md` Rules。
   - 输出 `specs/task-<name>.spec.md`, 所有导入 scenario 的 `Test:` selector 留空并标 `<!-- NEEDS CLARIFICATION -->`。
   - 导入完成后, Spec Kit 文件即作废, 真相库切换到 `specs/`。

2. `agent-spec import openspec <change-dir>`:
   - 读取 OpenSpec `proposal.md` / `specs/<capability>/spec.md` / `design.md` / `tasks.md`。
   - `### Requirement` → `Rule:` 行(id 用 OpenSpec slug, name 用人类标题), scope 落 `Capability(<capability-name>)`。
   - `#### Scenario` → agent-spec `Scenario:`(Test: 留空 + NEEDS CLARIFICATION)。
   - `proposal.md` Why → task spec 的 `## 意图`。
   - 输出与 `import speckit` 一致。

3. `agent-spec export openspec-verification <task-spec>`:
   - 把 lifecycle 结果写成 OpenSpec change 的 `verification.json` artifact。
   - 给仍然用 OpenSpec 做 archive merge 的团队提供"机器验证通过才允许归档"的过渡接口。

4. **高级模式**(可选, 不推荐稳定使用): `agent-spec lifecycle --capability-source-readonly <openspec-specs-dir>` 把 OpenSpec specs 作为 capability 真相库的只读 fallback, 让验证层先跑起来、迁移再做; 适合**渐进迁移**, 但真相分两处会漂移。

**框定**(避免叙事漂回"互补"路线):

- 三个命令都是**单向**: 导入后不双向同步, 导出后不读回。
- 预期使用路径是**一次性迁移**, 不是长期共存。
- 长期同时维护两套或三套 spec 真相库, 与 §0 / §9 / §12 / §13 的 BDD-spine 吸纳叙事直接冲突, 不在推荐范围。
- 这些命令不阻塞 Phase 1–4。属于 Phase 5+ 生态兼容工作, 可在核心 BDD 路线落地后再做。

---

## 12. 市场叙事建议

OpenSpec README 有 "How we compare" 叙事, Spec Kit 占据 GitHub SDD toolkit 的心智。agent-spec 的叙事不是"我跟它们是不同类别", 而是 **"我用 BDD 把三层做完整, 你不需要再装第二个工具"**。

更清晰的中英文叙事:

> agent-spec 是 BDD-native 的 spec-driven SDLC。
> Discovery 用结构化产物, Formulation 用 Rule → Example, Automation 用 Test selector + lifecycle gate。
> 一个聚合的 Task Contract 走完 OpenSpec 与 Spec Kit 加起来要走完的流程。

```text
agent-spec is the BDD-native spec-driven SDLC.
One aggregated Task Contract carries Discovery (questions), Formulation (Rule → Example),
and Automation (Test selector + lifecycle + boundary gate).
You don't need OpenSpec for living specs or Spec Kit for SDD scaffolding —
the same BDD primitives subsume both, without the directory split or phase pipeline.
```

一句话定位:

> One contract. Three layers. Mechanically verified.
>
> 一份合约, 三层闭环, 机器证明。

### 12.1 相对 sensor 派的差异化:统一 verdict 通道

Thoughtworks 的 [Maintainability sensors for coding agents](https://martinfowler.com/articles/sensors-for-coding-agents.html) 指出 AI 时代代码 maintainability 需要外置传感器。agent-spec 完全同意这个判断, 并把它再推进一步:**所有传感器的结果必须汇入同一个 verdict 通道**。

她的方案里 ESLint / dependency-cruiser / coupling 分析 / modularity review 是**各自独立**的工具——工程师 review 时要在脑里合成多份独立报告;agent-spec 的方案里 lint / boundary verifier / test verifier / ai verifier 都汇入同一份 `VerificationReport`, `is_passing` 公式统一, `explain` 输出是一份完整文档。

这件事对 PR review 与 liability 归属意义重大: 工程师为 AI 写的代码背书时, 不应该需要合成多份独立报告; 他应该看到一份 `explain` 输出, 告诉他覆盖矩阵全绿、五维质量分都过、边界没越界、所有 inferential verdict 都有可审查的 decision 链。

这不是和 Thoughtworks 派竞争, 而是把她那套方法论的工程缺口补上——**她提供 sensors 的概念与实操经验, agent-spec 提供统一通道的工具实现**。

### 12.2 中文双语 DSL 作为结构性壁垒

agent-spec 在数据模型层(§5.3 `SpecMeta.lang`)与 parser 实现层同时承担**中英双语 authoring**。这件事不是文案翻译, 是结构性的产品壁垒。

中文工程师用母语写可执行契约时, **业务意图表达精度比用英文高一个数量级**: `场景: VIP 客户的全额退款保持原币种` 在中文业务上下文里比 `Scenario: Full refund for VIP customer preserves original currency` 精确得多。这种精度差异在金融、政务、本地化业务的合约编写里直接放大成代码质量。

英文 spec 工具(OpenSpec / Spec Kit)的整个生态——文档、模板、示例、社区讨论——都是英文。他们没有动力做中文本地化; 即便做了, 翻译层仍然是 second-class citizen。agent-spec 的中文是 first-class: DSL 关键字、AST 字段、parser、lint、CLI 输出全链路双语。

这是英文工具结构上无法夺走的位置——不是"中国本地化", 是"中文工程师从 prickle 训练到契约表达的母语链路"。

### 12.3 README 段落（current 标记 v0.2.x 已交付的能力；planned 标记 BDD-spine 路线图未来阶段，详见 §13）

更完整的 README 段落:

```markdown
## How agent-spec compares

agent-spec is a BDD-native spec-driven SDLC. Where Spec Kit and OpenSpec
spread the BDD loop across multiple commands, files, or phases, agent-spec
keeps Discovery, Formulation, and Automation inside one aggregated Task
Contract:

- Automation (current) — every Scenario binds to an explicit `Test:`
  selector; lifecycle and guard fail on failed, skipped, or uncertain
  results; boundaries are mechanically checked against the change set.
- Formulation (Phase 1 in flight; Phase 3 planned) — `Rule: <id>` groups
  Scenarios at task scope today; the same Rule primitive lifts to
  project (constitution) and capability (living spec) scope in Phase 3.
- Discovery (Phase 4, planned) — `## Questions` + `<!-- NEEDS
  CLARIFICATION -->` markers will let lint block unresolved questions in
  strict mode.

OpenSpec's living capability specs and Spec Kit's constitution governance
both collapse into project- and capability-scope Rules in the same DSL.
No four-artifact split, no seven-phase pipeline, no thirty-integration
distribution machinery — just one contract, three layers, mechanically
verified.
```

---

## 13. 最终判断

agent-spec 的根基决策经三方对比后是成立的, 在 BDD-spine 吸纳路线下进一步强化:

1. **Task Contract 聚合式设计是优势。** 不要为了模仿 OpenSpec 拆 proposal/design/tasks。
2. **显式 Test selector 是护城河。** 它让 coverage 从 LLM 推断变成机械映射。
3. **`skip != pass` 是产品信念。** 这是 agent-spec 和文档型 spec framework 的分水岭。
4. **project/org inheritance 应继续加强。** 它是比 constitution 更可执行的治理层。
5. **Rule 是可提升的同一种基元(架构 keystone)。** 同一种 DSL 概念在 task / capability / project 三个 scope 都成立, 用它就能把 Spec Kit 的 constitution 与 OpenSpec 的活规格库统一收掉, 不需要复制对方的目录与流水线。`RuleKey = { scope, id }`: id 稳定, name 显示, 提升只改 scope。
6. **中英双语 DSL 是被低估的差异化。** 它让中文团队可以用母语写可执行契约, 而不是只翻译英文模板。
7. **caller mode 是使用 LLM 的正确边界。** 它不把 LLM 输出当成天然完成信号, 而是把 AI 判断变成可审计证据。
8. **BDD 三层闭环(Discovery → Formulation → Automation)是统一脊柱。** agent-spec 在一个聚合 Task Contract 里同时承担三层, 不需要分裂成三个工具; 也不需要把自己定位成"两者的下游验证层"。

这些优势成立的前提, 是接受 §9.4 的三项代价: agent-spec 价值曲线起步晚, `Test:` selector 会耦合测试标识符, NFR 需要专门 runner、外部探针或结构化 AI / caller 证据补齐。

因此, 后续演进方向不是"做成另一个 spec framework", 也不是"和它们互补", 而是 **用 BDD 把三层做完整**:

```text
Phase 1  Rule → Example 基元 + agent-readable lint guidance + lint-ack 语法预留   (specs/task-bdd-semantics-v1.spec.md)
Phase 2  机械覆盖矩阵 + ScenarioResult.evidence_provenance(computational vs inferential)   (Phase 3 / 4 的共同数据底座)
Phase 3  capability 层 + promote + Rule provenance event log + capability 依赖图   (OpenSpec 活规格库的 BDD-native 形态)
Phase 4  Discovery 结构化产物 + NEEDS CLARIFICATION         (Spec Kit /clarify 轻量等价, 同步改 Section enum / parser / lint / skills)
Phase 5  Lint 五维 + project.spec 冲突恒 critical + 完整 lint-ack 机制 + --inferential-policy + 跨 spec 矛盾检测   (Spec Kit /analyze + /checklist + constitution governance)
Phase 6  单源多工具生成                                     (替代手工维护的 install-skills.sh)
Phase 6.5 Probe 抽象前置设计                                (Example → Probe, 把 Test / Static / Benchmark / External / Inferential 统一)
Phase 7  跨语言 test runner + StructuralRule + 外部探针     (基于 Phase 6.5 Probe 抽象, 解决 §9.4 NFR 天花板)
Phase 8  agent-spec audit                                  (慢节奏体检; multi-run caller mode 默认在此 phase 开启)
Phase 9  agent-spec discover --from-codebase               (冷启动反向能力, 解决 §9.4 第 1 项代价; 命名与 init 区分)
       = BDD-native spec-driven SDLC, 一个聚合契约走完三层
```

各 phase 完成后, 在 `docs/` 下产出 `phase-{N}-retrospective.md`(模板见 `docs/phase-retrospective-template.md`), 把 before / after 真实数据沉淀下来。复盘是契约性 deliverable, 但**真复盘是独立 task, 不卡在源 phase 的完成条件里**——避免没数据的 retrospective 变成 theater。
