# robrix2 反馈清单 triage（2026-08-19）

来源：robrix2 `docs/agent-spec-feedback-2026-08.md`（24 条，基于 agent-spec 1.4.0）。
本文是对照源码逐条核实并经二次审查后的**实施清单**，取代原反馈中的优先级。

约束（不可违反）：
- **skip ≠ pass**（AGENTS.md:518）：任何改动不得提供把普通 `skip` 降级为通过的开关。
- **ADR-001**：判定记录不得携带身份字段（`trace_ledger.rs:138` 机械守卫）。

## 队列 1 — 确定性数据损失 / 解析 bug（可直接实施，建议 1.4.1）

| # | 结论 | 根因位置 | 修法 |
|---|---|---|---|
| B1 | 确认 | `spec_verify/boundaries.rs:136-148` 用"看起来像路径"白名单筛选条目，只认 `/ * .rs .ts .js .py .md .spec`（`.json/.toml/.yml/.txt` 全丢；`` `CLAUDE.md` `` 因结尾是反引号也丢；`Makefile/Dockerfile/LICENSE/Justfile` 无扩展名，任何后缀规则都救不了）；`plan.rs:113`、`spec_mcp/tools.rs:290` 三份逻辑不同步 | **放弃启发式筛选**：`### Allowed Changes` / `### Forbidden Paths` 下的条目按定义就是路径表达式——去反引号 → 剥尾注 → 规范化（`./`、`\`、裸文件名=仓库根相对）→ 直接参与匹配；无法解析为路径的条目由 lint `boundary-entry-shape` 告警而非静默丢弃。自然语言边界归 Constraints / 通用 Boundaries。三处合一为单一 `normalize_boundary_pattern` |
| B2 | 确认 | `spec_parser/parser.rs:296-302` 整行入 `Boundary.text`，`src/vcs.rs (new file)` 成为永不匹配的活 pattern | 尾注语法**明确限定**：只识别反引号跨度之外、前有空格的 `—` 或 `#` 分隔符及其后的说明；`(…)` 不剥离（可能是合法文件名的一部分），由 `boundary-entry-shape` lint 提示改写。剥离只作用于非反引号部分 |
| B5 | 确认，升 P1 | `spec_parser/parser.rs:261-268` `parse_string_list` 丢弃续行；影响 json/text/md 与 3 个 coverage linter | 解析器合并续行；缩进子 bullet 保持嵌套而非提升为同级 |
| C1 | 确认 | `main.rs:2100-2131` `upsert_capability_rule` 只写注释 + `### Rule:`；新建能力 spec 固定中文段名 `## 意图`/`## 完成条件` | 搬运 Rule 下全部场景块（Tags/Test/Package/Filter/步骤）；段名跟随源 spec 语言（或 `--lang`） |

## 队列 2 — 诊断质量与展示

| # | 结论 | 根因位置 | 修法 |
|---|---|---|---|
| C2 | 确认 | `test_verifier.rs:84-89` 非成员 package 与编译失败共用同一 reason；cwd 取 `--code` 向上首个 Cargo.toml | 先 `cargo metadata` 校验成员，报 "package not in workspace"；`Manifest:`/`Dir:` 选择器另立项 |
| A5 | 确认 | boundary 合成 `ScenarioResult`（`boundaries.rs:142-155`）计入 `from_results`（`verify.rs:205-237`） | 汇总分层：`scenarios{…}` 与 `layers{lint,boundary,test}` 分列 |
| A4-help | 确认 | `main.rs:256` guard help 只写 staged/worktree，实际接受 none/jj | 修 help |
| D1-checkpoint | 确认，**正确性问题，前移至本队列** | `Checkpoint`（`spec_core/verify.rs:130-142`）无 `spec_fingerprint` → `--resume incremental` 会跳过 spec 已改动的场景 | checkpoint 记指纹，不匹配即作废 |

## 队列 3 — manual verification（A1 + A2，需 LEP/ADR，一起做）

- 现状：`Review: human` 存在但要求测试真实运行且通过（`test_verifier.rs:233`）；零匹配 → `Skip`（`:231`）；无 manual 一等语义。
- 现有人工判定链路（`verify --emit-questions` → `resolve-ai`）只覆盖 `uncertain`/`pending_review`，**覆盖不了不存在测试产生的 skip**，因此 A2 依赖 A1，不能单独作为小补丁。
- 设计要点：
  1. 新增一等 manual 场景（如 `Verification: manual`），产生独立 verdict（`manual_pending`），不复用 `skip`。
  2. manual 场景由现有 answered-envelope 链路结算，判定进 run log（当前 `resolve-ai` 不调 `write_run_log`），并绑定 `spec_fingerprint`（run log 已有，`main.rs:3339`）失效。
  3. 记录内容：judgment 来源类别、verdict、reasoning、evidence digest、spec fingerprint、**外部审批系统引用**；不做 `--by`。
  4. `explain/stamp/matrix` 显示 manual 结算状态。
  5. **不提供** `--fail-on fail` 之类把普通 skip 降级为通过的选项。
  6. **迁移路径**（同一 LEP 内）：robrix2 有 45 个 spec / 数百个 `manual_test_*` skip，新语义上线后旧仓库不会自动变绿。需要：lint 对"选择器零匹配且名字形如 manual"的场景提示改用 `Verification: manual`；提供 `agent-spec migrate --manual-prefix manual_test_` 之类 codemod。
  7. 外部审批引用：核心记录只允许保存**不可解释的 external reference / digest**（PR review URL、commit sha 之类指向外部系统的引用）；审批人身份与时间由引用目标系统呈现。robrix2 现行 trailer "verified by X at T" 是身份+时间声明，不是引用，不得作为自由文本进入核心记录（否则等于绕过 ADR-001）。

## 队列 4 — CI / change ownership（A3 + A4 + C6，独立 LEP/ADR）

- 缺口成立：`main.rs:2845-2885` 同一 change set 交给每个 spec。
- 但"Allowed Changes 与变更集有交集才查边界"有循环定义风险：完全不相交恰可能是严重越界；spec 文件本身通常不在自己的 Allowed Changes 里；capability/project 与 task 适用范围不同；一个变更可能受多个 spec 治理。
- 先定义 spec ↔ 变更的 ownership/binding 规则，再实现 `--boundary-scope`、`--change-scope base:<ref>`、`agent-spec ci`。robrix2 `scripts/spec-guard.sh` 是参考流程，不是参考语义（它只用"spec 文件被改"当 engaged）。
- LEP 起点规则（robrix2 侧提议）：
  - spec 被变更集 **engaged** 当且仅当：变更集含该 spec 文件本身，或命中其 Allowed Changes 任一路径，或命中其 `### Symbols`；
  - engaged 的 task spec：用完整变更集做边界检查（现行语义）；
  - 未 engaged 的 task spec：只跑绑定测试（回归）；
  - capability spec 不做边界（长寿命，不"拥有"变更）；project spec 的 Boundaries 是约束不是路径；
  - 额外产出 **unowned changes** 报告：变更集中不属于任何 engaged spec 的 Allowed Changes 的文件——先可见不失败，由 project.spec 决定是否 opt-in 成失败。这就是"不相交≠无关"的处理方式。

## 队列 5 — 指纹漂移其余部分（D1）

- checkpoint 指纹已前移到队列 2。剩余：run log 的 `spec_fingerprint` 只有 `archive` 消费；`explain --history` 应标注 spec 内容变更；`stamp` 增 `Spec-Fingerprint`。`--spec-frozen` 另议。

## 队列 6 — 需最小复现后再定（B4、B6）

- B4 **已处理**：robrix2 样例（`task-dm-encryption-default.spec.md:27,38`）均为反引号内 Rust 返回箭头 `…) -> bool`。规则收紧为三条件同时满足才视为签名：箭头位于反引号代码跨度内、跨度内只有一个 `->`、左侧以 `)` 或 `|`（闭包）结尾且右侧是类型形状 token（原始类型 / 首字母大写 / 含 `<&(::`）。这样 `memory() -> disk`、`resolve(local) -> remote` 仍报警（右侧非类型），`|x| -> bool`、`lookup(k) -> Option<&V>` 不报警。回归测试 4 个方向各 1 条（`test_precedence_fallback_coverage_*`）；本仓库 `task-clause-coverage.spec.md:18` 误报同时消失。
- B6：无 level/Review/tags 门控；本仓库 36 条诊断多为 `输入`/`选择` 误报。等 A1 的 manual 语义落地后再决定放宽范围。

## 队列 7 — roadmap 立项（A6–A9、D2、C3、C5、D3）

- A6 identifier-drift（lint 需引入 code 输入）、A7 property kind（`Probe` enum 有保留位）、A8 Invariant 段、A9 mutate、D2 baseline 棘轮、C3 `check-structure` 多规则、C5 config.yaml 未被读取且被 gitignore、D3 `Spec-Base`（stamp 非 dry-run 尚未实现）。

## 与 robrix2 的往返（已同步至其 PR #329）

- **C4 误诊**（robrix2 侧已在 PR #329 更正）：`trace --gate` 现场调用 verification rollup（`spec_knowledge/trace.rs:60`），不依赖 `.agent-spec/runs`；代价是 `spec-guard.sh` 第 3 步 lifecycle + trace 双跑 cargo test。可考虑 opt-in 的 `trace --from-run-log`（roadmap）。
- **A2 的 `--by`**：与 ADR-001 冲突，审批人身份由外部系统按 evidence digest 绑定。
