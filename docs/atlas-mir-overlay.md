# Rust Atlas MIR Overlay

Rust Atlas 的 MIR 层是非默认、可逆的 compiler-evidence overlay。默认构建仍只依赖
stable Rust、Cargo metadata 和 `syn`。纯 JSON protocol consumer 不依赖 compiler crate，
因此参与默认编译和测试；只有公开的 `atlas build --mir/--mir-driver` 激活面由 Cargo
feature `mir` 控制。

## 边界

Atlas 不把 nightly compiler crates 链接进默认 binary。MIR producer 是独立进程，输出
版本化的 `rust-atlas/mir-overlay-v1` JSON。Atlas 负责验证 snapshot、解析 canonical
symbol、写入 `mir` call edge/CFG summary，并独立管理 capability 和 freshness。

Charon 在 2026-07-20 的评估中未通过本项目的兼容性门：它使用独立 nightly pin，不能
满足“支持仓库 stable toolchain 且滞后不超过两个 minor”的要求。目标 producer 因而是
单独发布、单独钉住 toolchain 的 `rustc_public` driver。`rustc_public` 本身仍是 WIP 且完全
unstable，所以 producer 不属于 agent-spec 默认 workspace，也不能成为 stable 构建前置条件。

当前仓库交付 MIR protocol、consumer、driver process adapter 和 fake-driver integration
tests；并未随仓库发布官方 `rustc_public` producer binary。producer 发布前，使用
`--mir <artifact>` 接入兼容产物；`--mir-driver` 是固定进程协议的集成点，不代表任意
producer 自动具有 compiler authority。

## 使用

```bash
cargo build --features mir

# 消费已经生成的 overlay
cargo run --features mir -- atlas build \
  --code . \
  --graph .agent-spec/graph \
  --mir target/rust-atlas/mir-overlay.json

# 调用外部 producer；Atlas 直接执行固定 argv，不经过 shell
cargo run --features mir -- atlas build \
  --code . \
  --graph .agent-spec/graph \
  --mir-driver /absolute/path/to/rust-atlas-mir-driver
```

driver 收到且只收到以下协议参数：

```text
rust-atlas-mir-driver --code <canonical-root> --out <graph>/mir-overlay.json
```

`--mir` 与 `--mir-driver` 互斥。producer 非零退出、文件缺失、JSON/schema 错误、source
fingerprint 不匹配、symbol 缺失或 CFG/call site 非法时，`atlas build` 仍返回成功的 syn
加可选 SCIP graph，同时输出 `mir-extraction-failed` warning 并清除旧 MIR facts。consumer
递归拒绝 extractor、CFG 和 site 等 nested object 的未知字段。完整 shard 集先写入 staging
generation，再通过目录交换提交；任一 staging write 失败都不会部分修改当前 generation。

## Artifact

JSON Schema 位于
[`docs/atlas-schemas/mir-overlay-v1.schema.json`](atlas-schemas/mir-overlay-v1.schema.json)。
最小 artifact：

```json
{
  "schema": "rust-atlas/mir-overlay-v1",
  "extractor": {
    "name": "rustc_public",
    "version": "nightly-YYYY-MM-DD"
  },
  "source_fingerprint": "<64 lowercase hex characters>",
  "functions": [
    {
      "symbol": "my_crate::service::run",
      "cfg": {
        "basic_blocks": 3,
        "edges": 2,
        "exits": 1,
        "loop_headers": 0
      },
      "calls": [
        {
          "target": "my_crate::store::load",
          "site": {
            "file": "src/service.rs",
            "line_start": 18,
            "column_start": 5,
            "line_end": 18,
            "column_end": 17
          },
          "dispatch": "static",
          "generic": false,
          "evidence": "rustc_public MIR terminator Call"
        }
      ]
    }
  ]
}
```

producer 必须使用 Atlas 的 source-set fingerprint 算法：按 repository-relative path 排序的
`{path: blake3(file-bytes)}` JSON map 再做 blake3。函数与 target 使用 Atlas canonical
symbol，并且必须解析到函数节点；consumer 不接受猜测式 suffix resolution。`dispatch` 为
`generic` 时 `generic` 必须显式为 `true`，其他 dispatch 的 `generic` 必须省略或为
`false`。

## Query And Freshness

canonical shards 保留 syn、SCIP 和 MIR 的原始证据。derived query index 对同一
source-target call/reference relation 只投影最高 provenance（`mir > scip > syn`），因此
`query`、`refs`、`flow`、`impact` 和 `explore` 共用同一 precedence，但不删除低层 shard
evidence。函数 CFG summary 只来自 MIR overlay。

`atlas status` 结构化返回 extractor identity、overlay recorded/current fingerprint 和 source
recorded/current fingerprint。syn refresh 后旧 MIR 可以继续作为 stale evidence 存在，但
`require_authority` 会阻止它成为 definitive binding、impact 或 lifecycle evidence。

参考：[rustc_public API](https://doc.rust-lang.org/nightly/nightly-rustc/rustc_public/)、
[rustc_public `run!`](https://doc.rust-lang.org/nightly/nightly-rustc/rustc_public/macro.run.html)、
[Charon](https://github.com/AeneasVerif/charon)。
