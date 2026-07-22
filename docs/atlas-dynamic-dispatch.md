# Rust Atlas Dynamic Dispatch

Rust Atlas 的动态分派层是显式启用的 whole-graph enricher。它不修改 `syn` parser，也不把
候选实现标成 compiler fact：原始 SCIP call 仍指向 trait method declaration，enricher 另加
一条 `unresolved`、`bounded-candidates`、`dispatch: trait` 的候选边。

```bash
agent-spec atlas build --code . --scip target/index.scip --dynamic-dispatch
```

## V1 Trait Dispatch

v1 只消费一个高精度 anchor：resolved SCIP `calls` edge 的 target 必须是 trait 内的函数。
Atlas 再通过 resolved `impls-trait` edge 找到实现该 trait 的 impl node，并按相同 method name
连接其函数节点。候选按 canonical node id 排序去重。

每个 call site 的上限是 64。超过上限时 Atlas 不返回前 64 个冒充完整集合，而是跳过该
inferred edge，并输出 `dynamic-dispatch-truncated` warning。没有 resolved trait-call anchor
时，这个 pass 是严格 no-op。

原始 SCIP declaration edge 始终保留。候选 edge 使用
`rust-atlas-dynamic-dispatch v1` extractor identity、继承 anchor 的 source site，并在
`evidence` 中记录 trait method 与候选数量。flow、impact 和 explore 通过现有 unresolved
candidate contract 遍历这些实现。

若同一 caller 到 implementation method 已存在 exact edge（例如 MIR evidence），派生 query
index 的 incoming/outgoing adjacency 选择 exact edge；bounded edge 仍留在 canonical shard
中作为来源可追溯的候选证据。

## Deferred Mechanisms

closure/function pointer、async spawn、channel、callback registry 与 framework route 需要各自
的 mechanism gate、corpus 和 false-positive 检查。它们应作为独立 whole-graph plug-in
加入，不能在 core parser 中按名称全局猜测，也不能复用 MIR provenance。
