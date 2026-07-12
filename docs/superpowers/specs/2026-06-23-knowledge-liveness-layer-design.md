# 设计:Knowledge & Liveness Layer (KLL)

> **Status:** Draft
> **Date:** 2026-06-23
> **Topic:** 在 agent-spec 中加入持久知识层、`Satisfies` 边、派生 liveness、只读 MCP、治理 lint 与固定目录脚手架
> **Scope:** 完整设计(A3),分 P1/P2/P3 交付

---

## 1. 摘要

把 agent-spec 从"代码验证器"升级为 **「知识 → 规格 → 代码」端到端可追溯 + 治理引擎**。新增:

- **持久知识 artifact**(`decision`、`requirement`,后续 `proposal`),采用 Lore/RAC 文档工程的 typed-section 形态;
- **`Satisfies` 边**:由 spec 声明它实现了哪条决策/需求,引擎建反向索引;
- **派生 liveness**:回答"这条决策/需求现在还被代码守着吗",复用现有 verify/coverage 裁决引擎,**永不落盘**;
- **只读确定性 MCP server**:把知识 + spec + 当前 liveness 喂给 agent("引用而非违反",且带代码真值);
- **治理 lint**:把 RAC/Lore 的图卫生与需求质量严谨度泛化到 `spec_lint`;
- **固定目录脚手架**:`init --workspace` 生成 Lore 形的知识目录树。

全程 **additive**——不改变 spec 现有裁决语义(agent-spec 金律)。

---

## 2. 背景与动机

### 2.1 反转:agent-spec 已经是 `satisfies` 验证器

跨工具调研(Epic Lore 文档工程、RAC/requirements-as-code、agent-spec)得出一个核心判断:

- **RAC** 把团队知识(决策/需求)存为 typed Markdown、只读 MCP 喂给 agent、确定性检索、`gate` 校验图卫生。但其 schema **结构性地不支持** `satisfies`/到代码的边——只能保证知识图*内部自洽*,无法回答"代码是否仍守着这条决策"。
- **agent-spec** 的 Task Contract 把 `Completion Criteria`(BDD)经 `Test:` 选择器绑定真实测试,`verify/lifecycle/guard` 真跑 `cargo test` + 边界/结构检查并裁决。**它正是"代码是否满足契约"的机器验证器**——即 RAC 缺的那条边的代码侧引擎。但 agent-spec 明确"spec 即唯一真相源、不外链 ADR/需求"。

两者是**同一条断链的镜像两半**:RAC 有知识、缺代码边;agent-spec 有代码验证、缺持久知识。KLL 接上中间那条边:

```
决策/需求  ──Satisfies──▶  spec(.spec.md)  ──Test:──▶  cargo test  ──▶  代码
(持久、不可变)              (可验证契约)                   (裁决)
        ▲                                                              │
        └────────────── liveness 上卷(决策是否被代码守着) ◀────────────┘
```

### 2.2 要解决的具体问题

1. agent-spec 没有"决策/需求"这层持久知识——决策只是 task spec 内联的 `## Decisions`,不具备 org/项目级、不可变、可 supersede 的记录形态。
2. 无法回答"决策 ADR-001 现在还被代码守着吗"——这是 RAC 与 agent-spec 单独都答不了的问题。
3. agent 缺一个确定性、只读、带**当前代码真值状态**的知识检索面。

---

## 3. 目标 / 非目标

### 3.1 目标

- 在 agent-spec 内新增原生持久知识 artifact 类型,带 Lore/RAC 文档工程严谨度。
- 提供 `Satisfies` 边 + 派生 liveness,使"决策被代码守着"成为**可计算、可 gate**的属性。
- 提供只读确定性 MCP server。
- 提供固定目录脚手架(Lore 形)。
- 全程 additive,不破坏现有 spec 语义与既有用法。

### 3.2 非目标(对设计自身用「负空间」划界)

- **不**搬入 Lore 的产品文档族(Diátaxis:tutorial/how-to/reference/explanation)。
- **不**做 Lore 的 prose-style canon(voice、句子式标题、禁用语、em-dash 间距等散文风格 lint)——agent-spec 不是文档站生成器。本设计里的 `canon/` 指 **artifact-schema canon**(决策/需求该含什么),不是 prose canon。
- **不**做 RAC corpus importer(RAC 非普及工具,迁移成本不值)。
- **不**改变 spec 的 `pass/fail/skip/uncertain/pending_review` 裁决语义;liveness 是其上的派生上卷。

---

## 4. 已定决策(含理由)

| # | 决策 | 选择 | 理由 |
|---|---|---|---|
| D1 | 知识形态 | **agent-spec 原生新 artifact 类型**(非复用 RAC 格式、非塞进 BDD Rule) | 决策给 MADR、需求给 EARS,各得其形;一套 parser/AST/lint/report;不造两个真相源;不把"Rule"(行为分组)语义overload |
| D2 | liveness 引擎 | **复用现有 verify/coverage 裁决引擎**上卷 | ②的正确建模 + ①的近乎白捡的 liveness;additive |
| D3 | `Satisfies` 边方向 | **由 spec 声明**,引擎建反向索引(decision→specs) | 持久决策不被易变 spec 反向耦合(守不可变原则) |
| D4 | 提案 | **MVP 不做独立类型;用决策的 `Proposed` 状态兜底;独立 `proposal` 类型留作 P3 治理 phase** | 提案是 pre-decision、无代码可验、liveness 恒 N/A——离 agent-spec 核心动词最远,收益最低 |
| D5 | RAC importer | **不做** | RAC 非普及;无语料要保留,更坐实"原生类型"选择 |
| D6 | 集成方式 | **在现有单一 binary crate 内新增 module(`mod spec_knowledge` / `mod spec_mcp`),进程内调 `SpecGateway`**(非子进程、非新 crate) | agent-spec 是单一 binary crate(`spec_core` 等皆为 `crate::` 下的 `mod`,`Cargo.toml` 无 `[workspace]`);新增 module 与之同构,延续其模块分层 |
| D7 | liveness 落盘 | **永不落盘,按需派生** | 确定性、可复现(RAC 原则);防漂移(Lore 不可变) |
| D8 | 给 AI 的指导/skill | **typed `guidance` 类型(治理档,`liveness:n/a`)+ 自由格式 `context/` 逃生舱(serve 不 lint)** | typed 才能让 agent 稳定消费、合七原则;逃生舱给"不想写 schema 随手丢 context"留出口,代价是无治理 |

---

## 5. 七原则吸收对照(Lore 文档工程)

| Lore 原则 | KLL 中的落地 |
|---|---|
| ① 分类消歧 | `kind: decision\|requirement`;`knowledge/<kind>/` 目录定类型;type-driven schema |
| ② 模板降方差 | 每 kind 一份 `*-template.md`(`init --kind` 复制即用);required/recommended 节 |
| ③ 负空间防过度 | schema 明列"不支持什么"、禁即兴 frontmatter;N/A 须解释;Accepted 决策 Alternatives 不得为空;§3.2 对设计自身划界 |
| ④ linter 闭环 | 治理 lint 泛化 `spec_lint`;单 SARIF→Code Scanning;**讲规则的 standards 文档自身豁免 artifact lint**(self-referential exemption);三态(pass/findings/errored)沿用 |
| ⑤ 量化证据撑决策 | **headline:liveness = 决策的*持续* Confirmation**。Lore 的 Confirmation 是 decision-time 一次性 benchmark(会腐烂);KLL 把它升级为"`Satisfies` 的 spec 的实时裁决"——`honored` 即"此刻仍被机器证明成立"。这是 agent-spec 独有、RAC 给不了的能力,也是本设计对原则⑤最深的吸收 |
| ⑥ 不可变 + supersede | `Status` 生命周期 + `## Supersedes` + supersession-integrity lint |
| ⑦ 分级用力 | `.agent-spec/config.yaml` 渐进 gate 策略(`violated→error` / `unproven→warning`,day-one 可关);change-scope 感知;per-rule 开关(gate-green-day-one) |

---

## 6. Artifact 模型

复用现有 `spec_parser`/AST,新增 artifact kinds,放 `knowledge/`。frontmatter 仅承载身份:

```yaml
---
kind: decision            # decision | requirement | (P3) proposal
id: ADR-001               # 稳定,走现有 id-resolution(NNNNN-slug 前缀亦可解析)
status: Accepted          # 见各 kind
supersedes: ADR-000       # 可选
liveness: auto            # auto(可验证) | n/a(治理决策,如许可/策略,永不进代码 gate)
---
```

### 6.0 id 解析(P1 必须定清)

`trace` 与反向索引都依赖确定的 id 解析。P1 规则(按优先级):

1. frontmatter `id:` 字段为**规范来源**。
2. 缺省时回退到文件名前缀 `<letters>-<digits>`(`adr-001-soft-delete.md` → `ADR-001`)。
3. 匹配**大小写不敏感**,内部规范化为大写前缀 + 数字(`adr-001` ≡ `ADR-001`)。
4. **冲突**(两文件解析出同一 id)= lint `error`,阻断。

前缀注册表(`ADR-`/`REQ-`/`LEP-` 与各自目录的对应)写在 `standards/operational`;**解析算法本身在本设计固定,不外推**。scaffold 产出的实例始终携带 frontmatter `id:`(走规则 1),故 `NNNNN-slug.md`(数字起头)文件名无需依赖回退语法即可解析。

### 6.1 decision(MADR 形)

- **必填**:`## Context` · `## Decision` · `## Consequences`
- **推荐**:`## Status` · `## Category` · `## Alternatives Considered`
- **可选**:`## Supersedes` · `## Related`
- `Status`:`Proposed | Accepted | Superseded | Deprecated | Rejected`(`Proposed` 兼任"提案",见 D4)
- **forcing functions**(lint):Accepted 决策的 `Alternatives Considered` 不得为空;`Consequences` 须含正反两面(Good/Bad because)

### 6.2 requirement(EARS/规范形)[P2]

- **必填**:`## Problem` · `## Requirements`(`[REQ-NNN] … MUST/SHOULD/MAY …`,一行一条)
- **推荐**:`## Success Metrics` · `## Risks` · `## Assumptions`
- 需求质量 lint(可开关,gate-green-day-one):BCP-14 规范关键词、ISO/IEC/IEEE 29148 单一语句、EARS 句法

### 6.3 proposal [P3]

- 独立治理类型;走 lifecycle + forcing-function lint;`liveness` 恒 `n/a`,永不进代码 gate;`## Produces: ADR-xxx` 链到它催生的决策(补 Lore LEP→ADR 交接的 open question)

### 6.4 guidance(给 AI 的指导 + skill 指定,治理档)[P2]

- **必填**:`## Scope` · `## Instructions`;**推荐**:`## Applies To`(路径/栈 glob)· `## Constraints`(Must/MustNot)· `## Skills`(本场景指定的 skill)· `## Examples`。
- 按栈作用域:子目录(`guidance/rust/`)或 `tags: [rust]`。
- `liveness` 恒 `n/a`;不验证代码、永不进代码 gate。
- **消费**:经现有 `gen-integrations` 投影成各工具原生格式(`CLAUDE.md`/`.cursor/*`/`AGENTS.md`/skill 配置)+ MCP 实时 `guidance.for(path|stack)`。

### 6.5 context(自由格式逃生舱,非 artifact)

- `knowledge/context/` 下任意 Markdown,**不 typed、不 lint、无 schema**。
- MCP **只读 serve**(`context.read(path)`),供随手投放 agent-context。
- 代价:不受治理约束、无模板保证——刻意的 escape hatch(D8),是 §9 负空间原则的权衡出口。

---

## 7. `Satisfies` 边 + liveness 引擎

**P1 解析面(固定)**:`Satisfies` 落在 **spec 的 frontmatter**,字段 `satisfies: [ADR-001, REQ-002]`(id 列表)。由 `spec_parser/meta.rs` 与现有 frontmatter 字段(`spec/name/inherits/tags/depends/…`)一并解析。引擎扫全部 spec 建反向索引 `decision_id → [spec]`。
- **粒度**:P1 仅 **spec 级**。scenario/rule 级延后,届时镜像现有内联 `Test:` 选择器(`spec_parser/keywords.rs` 加 `Satisfies:` 关键词),不改 P1 面。

**liveness 派生**(对 `liveness: auto` 的 artifact K,按**优先级阶梯**求值,保证四态互斥且穷尽):

> 术语区分:frontmatter 的 `liveness:` 是**声明字段**(输入,`auto`\|`n/a`,表是否参与代码验证);下面求出的是**派生状态**(输出,`honored`\|`violated`\|`unproven`\|`n/a`)。两者共用 `n/a`:声明 `n/a` ⇒ 派生状态恒 `n/a`。

1. 任一满足 K 的 spec 当前裁决 `Fail` → **`violated`**
2. 否则,若**无** spec 满足 K,或任一满足者为 `Skip`/`Uncertain`/`PendingReview` → **`unproven`**(`PendingReview` 表"尚未机器证明",不计入 honored)
3. 否则(所有满足者皆 `Pass`)→ **`honored`**
4. `liveness: n/a` 的 artifact 直接为 **`n/a`**,不进上述阶梯、永不 gate

- 底层 spec 裁决**复用** `spec_verify`/coverage(`Verdict` 枚举:`Pass/Fail/Skip/Uncertain/PendingReview`);liveness 是 `spec_report` 的派生上卷(additive,不改 spec 裁决语义)。
- 新命令:`agent-spec trace <id>`(反向 `--for-spec <name>`)→ 满足关系、各 spec 裁决、上卷结果;`--format text|json|md`。

---

## 8. Gate

- 扩展 `guard` / `lifecycle`,新增渐进策略(`.agent-spec/config.yaml`):
  - `violated → error`(失败)
  - `unproven → warning`(可 day-one 关闭)
  - `n/a` 永不 gate
- **change-scope 感知**:当代码改动落入某决策辖区(经其满足 spec 的 Boundaries),确保这些 spec 已跑。
- 产出**单个 SARIF**,一个 category → GitHub Code Scanning(薄壳厚引擎:CLI 即真相,action 不重释 findings)。

---

## 9. 治理 lint(Lore/RAC 文档工程严谨度)

泛化 `spec_lint` 到知识 artifact:

- 必填节存在性;
- **supersession 完整性**:禁引用已 superseded 的 id;`Supersedes` 目标须存在并被标记;
- 断链 / 歧义链接;
- 需求质量(EARS/BCP-14/29148,可开关);
- **负空间 schema**:明文枚举"不支持什么"(除 `Satisfies` 外不链任意文件;禁即兴 frontmatter 字段),lint 拒绝即兴;
- **self-referential 豁免**:`knowledge/standards/**` 与各目录 `README.md`(讲规则的元文档)对自身豁免 artifact lint。
- **`knowledge/context/` 不在治理范围**:自由格式逃生舱(D8),MCP 只读 serve 但不 lint、无 schema。

---

## 10. MCP server(只读、确定性)

- 新 module `spec_mcp`(`crate::spec_mcp`);命令 `agent-spec mcp`。无 RAG/embedding/模型调用判定相关性。
- 暴露的工具(确定性):
  - `knowledge.find(path|tag|id)`
  - `knowledge.governing(path)` → 管该路径的决策(经满足 spec 的 Boundaries) + **当前 liveness**
  - `liveness.status(id)`
  - `spec.contract(name)`
  - `guidance.for(path|stack)` → 管该路径/栈的 `guidance` 指导 + 指定 skills
  - `context.read(path)` → 只读 `knowledge/context/` 的自由格式 context
- 薄读层叠在 `SpecGateway` + trace 索引上,复用现有 JSON 契约。

---

## 11. Scaffold + 固定目录布局(Lore 形)

`agent-spec init --workspace`(幂等、只补不覆盖)生成:

```
<repo>/
├── knowledge/                       # ≈ Lore docs/developing/
│   ├── decisions/
│   │   ├── README.md                #   Landing:放什么 + 何时别放
│   │   ├── adr-template.md          #   模板(原则②)
│   │   └── NNNNN-slug.md
│   ├── requirements/                # [P2] README + requirement-template.md + REQ-NNN-slug.md
│   ├── proposals/                   # [P3] README + lep-template.md + YYYY-MM-DD-slug.md
│   ├── guidance/                    # [P2] 给 AI 的指导 + skill 指定(typed,治理档)
│   │   ├── README.md
│   │   ├── guidance-template.md
│   │   └── rust/                    # 可按栈分子目录(或用 tags)
│   ├── context/                     # 自由格式逃生舱:MCP 只读 serve,不 lint,无 schema
│   │   └── README.md
│   └── standards/                   # ≈ Lore doc-standards(artifact-schema canon,非 prose canon)
│       ├── README.md                #   Authority 分层:canon > operational > tools
│       ├── canon/
│       │   ├── artifact-types.md     #   每 kind 必填/推荐/负空间(= Lore doc-types.md)
│       │   └── linking.md            #   Satisfies/Supersedes 语义(= Lore relationships)
│       └── operational/             #   命名约定、review-checklist、id 前缀
├── specs/                           # agent-spec 现有,不动
└── .agent-spec/
    ├── config.yaml                  # gate 策略 + 渐进开关 + 路径(tools 层)
    └── runs/
```

- 每 `knowledge/<kind>/` = Landing README + `*-template.md` + 实例(照 Lore 三件套)。
- 命名照 Lore:决策 `NNNNN-slug`、提案 `YYYY-MM-DD-slug`。
- **固定布局是确定性的载体**:路径固定约定 → MCP 的 `governing(path)`、gate 的 change-scope→决策辖区映射,确定可复现、零猜测。

`.agent-spec/config.yaml`(P1 最小骨架;`.agent-spec/` 目录已存在于 agent-spec 运行时,此为 additive 新增文件):

```yaml
paths:                       # 以下为内置默认值;改此处即覆盖布局
  knowledge: knowledge       # 知识根
  specs: specs               # spec 根(agent-spec 现有)
skills:                      # 全局默认 skill 指定(guidance 的 ## Skills 可场景化覆盖)
  - <skill-id>
liveness:
  gate:
    violated: error          # 失败
    unproven: warning        # 可设 off 实现 gate-green-day-one
governance:                  # per-rule 开关(P2/P3 充实)
  rules:
    requirement-ears: off
    requirement-bcp14: off
    supersession-integrity: error
```

### 11.1 默认即约定,用户可改

- **默认采用此固定布局,无需用户显式选择(convention over configuration)**:agent-spec 把上述路径作为**内置默认**;用户不"选择启用",默认就按这套布局工作。`init --workspace` 只是把默认结构**物化**到磁盘(模板、Landing、standards)。
- **可定制,但走入库配置**:要改布局,通过 `.agent-spec/config.yaml` 的 `paths:`(及目录命名约定)覆盖默认值。定制是**声明式、随仓库提交**的,因此检索/gate 的**确定性不受影响**——路径由 config 钉死,而非临时猜测。
- **不强制物化**:用户也可不跑 `init --workspace`、自带布局,只要在 `config.yaml` 指明 `paths:` 即被引擎识别。

---

## 12. 模块布局与集成

agent-spec 是**单一 binary crate**(`src/main.rs` 经 `mod spec_core; mod spec_parser; …` 声明,`Cargo.toml` 无 `[workspace]`/`members`)。KLL 与之同构,新增**模块**(非 crate):

- `crate::spec_knowledge` — artifact kinds、`satisfies` 解析、liveness 上卷、治理 lint 规则、scaffold。
- `crate::spec_mcp` — 只读 MCP server。

复用:`spec_core`(AST/verdict)、`spec_parser`、`spec_gateway`(进程内 `SpecGateway`)、`spec_lint`、`spec_verify`、`spec_report`。

新 CLI 子命令(`main.rs`):`trace`、`mcp`;`init` 新增子标志 `--workspace` 与 `--kind decision|requirement`(与现有 `init --level/--name/--lang/--template` 并存,非新命令);扩 `guard`/`lifecycle`/`audit`。

---

## 13. 分阶段交付

- **P1**:scaffold(`init --workspace`,至少 `decisions/` 三件套 + `standards/canon/artifact-types.md` + `.agent-spec/config.yaml`,并**占住 `guidance/`、`context/` 空目录 + Landing**)+ `decision` 类型 + `Satisfies` 边 + liveness 上卷 + `trace` + gate(`violated`/`unproven`)。**最小可验证「决策是否被代码守着」端到端。**
- **P2**:`requirement` 类型(EARS/BCP-14/29148 lint)+ `guidance` 类型(经 `gen-integrations` 投影 + MCP `guidance.for`)+ `context/` 只读 serve(`context.read`)+ 只读 MCP server。
- **P3**:完整治理 lint(supersession/forcing-functions 全开)+ SARIF→Code Scanning gate(**SARIF writer 为净新增,src/ 无既有可薄壳复用**)+ `proposal` 治理类型(含 `Produces` 边)。

每 phase additive,沿用 agent-spec Phase 文化(只加传感器/报告,不改裁决语义)。

---

## 14. 风险与开放问题

1. **与 agent-spec「spec 即唯一真相源、不外链 ADR」哲学的张力**——靠"原生 artifact 类型 + 全程 additive"化解,使知识层不是异物而是同族新类型。
2. **治理 vs 可验证两档必须显式**(`liveness: auto|n/a`),否则会对不可 gate 的决策误 gate。
3. **id 命名空间**:核心解析算法已在 §6.0 固定(P1);`standards/operational` 仅持前缀注册表与目录映射等约定,不承载算法。
4. **liveness 跨运行确定性**:可复用现有 `measure-determinism` 验证。
5. **`Satisfies` 边的粒度**:落在 spec 级 vs scenario/rule 级——P1 先 spec 级,按需下沉。

---

## 15. 验收标准

### 设计层

- [ ] D1–D7 决策被采纳,理由可追溯。
- [ ] 七原则①–⑦各有对应落地点(§5)。
- [ ] 非目标(§3.2)明确,防范围蔓延。

### P1 实现层(后续 writing-plans 细化)

- [ ] `agent-spec init --workspace` 幂等生成固定目录树。
- [ ] 默认布局**零配置可用**(无需用户显式启用);`config.yaml` 的 `paths:` 覆盖后被引擎识别,确定性不变。
- [ ] 能 parse `decision` artifact 并 lint(必填节 + forcing functions)。
- [ ] spec 的 `Satisfies:` 被解析,引擎建反向索引。
- [ ] `agent-spec trace <decision-id>` 输出满足关系 + 各 spec 裁决 + liveness 上卷。
- [ ] `guard`/`lifecycle` 在 `violated` 时失败、`unproven` 时按配置告警,`n/a` 不 gate。
- [ ] 不改变任何现有 spec 的裁决结果(additive 回归)。
