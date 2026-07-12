# 附录 D 端到端轨迹

单章讲的是模块，轨迹讲的是系统。两条 E2E trace 把分散的章节串成完整的因果链。

## 轨迹一：一条需求从 PRD 到 honored

**贯穿**：第 10 章（intake）→ 第 11 章（治理）→ 第 12 章（计划）→ 第 7 章
（lifecycle）→ 第 13 章（溯源）→ 第 15 章（liveness）。

```mermaid
sequenceDiagram
    participant PRD as PRD 文档
    participant IR as 需求 IR
    participant Plan as 计划 DAG
    participant Spec as 任务合同
    participant LC as lifecycle
    participant KLL as liveness
    PRD->>IR: requirements import(标记块→REQ-X, proposed)
    IR->>IR: 人类 transition --to accepted(行精确改写+digest)
    IR->>Plan: graph --gate → work-units(WU-REQ-X ready)
    Plan->>Spec: draft-specs → 人审 → 提升 specs/(占位选择器换真实测试)
    Note over Plan,Spec: plan --gate 强制:accepted 必须有活跃合同
    Spec->>LC: Agent 实现 → lifecycle 重试循环 → N/N pass
    LC->>KLL: trace 记录落盘(带类型化代码目标)
    KLL-->>IR: requirements status REQ-X → accepted/verified/honored
```

每一步的产物都可独立审计：transition 的 JSON 带文档摘要（第 11 章）；plan 的
批次是拓扑序（第 12 章）；lifecycle 的 run log 记录每次重试（第 7 章）；
`traceability` 一个文档投影整条链（第 13 章）；最后 `status` 的三轴回答是
派生的，问一次算一次（第 15 章）。**没有任何一步依赖"某人记得"。**

这条轨迹在真实世界完整跑过：agent-spec 1.0 的三个集成需求
（REQ-COMPILER-MACHINE-SURFACE 等）就是沿着它从 proposed 走到
accepted/verified/honored 的。

## 轨迹二：一次合同验证之旅（含符号与边界）

**贯穿**：第 4 章（四要素）→ 第 6 章（lint）→ 第 7 章（四层管线）→ 第 8 章
（边界与符号）→ 第 16 章（Atlas 图）→ 第 9 章（验收与盖章）。

```mermaid
flowchart TD
    A["合同就绪(四要素+### Symbols)"] --> B["lint --min-score 0.7"]
    B -->|通过| C[StructuralVerifier<br/>Must NOT 模式匹配]
    C --> D[BoundariesVerifier<br/>变更文件×允许 glob]
    D --> E{声明了 Symbols?}
    E -->|否| F[TestVerifier 执行绑定测试]
    E -->|是| G{"atlas check:图新鲜?"}
    G -->|滞后| H["atlas-stale 失败<br/>(绝不假报 symbol-missing)"]
    G -->|新鲜| I{"每个符号在图中?"}
    I -->|缺失| J[atlas-symbol-missing<br/>逐符号 step verdict]
    I -->|全中| F
    F --> K{五种 verdict 汇总}
    K -->|is_passing| L["explain 验收 → stamp 盖章<br/>trace 记录带图指纹"]
    K -->|fail/skip| M[Agent 读证据修码重试]
    M --> C
```

注意两个设计细节如何在全链路里呼应：**stale 优先**（第 8 章）依赖 Atlas 的
blake3 新鲜度模型（第 16 章）；最终 trace 记录里的图指纹让"当时对着哪个代码
状态验证的"永远可答（第 13 章）。验收时人类读到的 explain 摘要（第 9 章），
背后是这整条机械链的汇总——这就是为什么两个"是"就可以放心批准。

## 轨迹揭示的原则

两条轨迹的共同点：**每个箭头都是一条命令，每个节点都有可校验的产物**。系统的
可信不来自某个环节的聪明，而来自链条上没有一环允许"口头承诺"。
