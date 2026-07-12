# 附录 A 命令速查表

基于 agent-spec 1.0.0（兼容性承诺内的全部表面）。

## 合同工作流

| 命令 | 用途 |
|------|------|
| `init --level task --lang zh --name N` | 脚手架新合同（`--template rewrite-parity` 用于迁移/对齐任务）|
| `parse <spec>` | 结构确认（段落数、场景数）|
| `lint <spec> --min-score 0.7` | 合同质量门 |
| `contract <spec>` | 渲染任务合同（Agent 的执行计划）|
| `lifecycle <spec> --code . --format json` | 主质量门：lint+结构+边界+测试（+符号）|
| `verify <spec> --code .` | 仅验证（跳过 lint 门）|
| `guard --spec-dir specs --code . --change-scope staged` | 全仓守卫（pre-commit/CI）|
| `explain <spec> --format markdown [--history]` | 合同级验收摘要（/运行历史）|
| `stamp <spec> --dry-run` | Git trailer 溯源 |
| `matrix <spec> --code .` | Rule×场景×测试×verdict×provenance 矩阵 |
| `audit --spec-dir specs` | 库健康度（只观察不阻断）|
| `promote <spec> --rule <id> --to <cap>` | 成熟 Rule 升能力库 |
| `discover --from-codebase --code . --name N` | 从测试反推草稿合同 |
| `archive --dry-run` | 完成合同归档（证据不绿则阻断）|
| `graph --spec-dir specs` | 合同依赖 DAG 与关键路径 |
| `check-structure --forbid X --in glob` | 分层架构守卫 |

## 意图编译（requirements 家族）

| 命令 | 用途 |
|------|------|
| `requirements import --from prd.md [--provenance m.json]` | 标记块/YAML → 需求 IR（proposed）|
| `requirements transition <ID> --to accepted [--format json]` | 显式治理转换（带摘要 JSON）|
| `requirements supersede <OLD> --by <NEW>` | 原子替换链 |
| `requirements status <ID>` | 三轴状态（治理/执行/liveness）|
| `requirements graph / plan --gate` | 需求图校验 / 三层计划 DAG |
| `requirements work-units / draft-specs` | 工作单元 / 合同草稿 |
| `requirements traceability <ID> --format json` | 证据链单文档投影 |
| `requirements verify-run --manifest m.json` | 编译重放，逐字节比对 |
| `requirements compile --out d/ --layout arc-v1` | per-requirement 四件套编译束 |
| `requirements bind` | 工作单元 × 代码符号绑定 |
| `requirements bundle --unit WU-X --out b.json` | 一体化执行束 |
| `requirements trace/replay/explain-failure <ID>` | 证据记录读取 |
| `requirements questions / test-obligations / worktrees` | 澄清问题 / 测试义务 / 并行工作树 |
| `requirements export --out r.yaml --check` | YAML 投影（漂移门）|

## 知识与生态

| 命令 | 用途 |
|------|------|
| `init --workspace` | 铺设 knowledge/ 工作区（幂等）|
| `trace <id> --gate` | 知识 → 合同 → liveness |
| `lint-knowledge --gate [--format sarif]` | 语料治理 lint |
| `atlas build/tree/query/refs/impls/check [--frozen]` | 代码图家族 |
| `wiki init/seed/status/query/inspect/lint/check` | live wiki 家族 |
| `mcp` | 只读 MCP server（11 工具）|
| `gen-integrations --check` | 集成文件漂移门 |
