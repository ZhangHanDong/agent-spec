# Requirements Note App Fixture

This compact Rust fixture demonstrates the intent compiler on an external
project. It is not the self-hosting dogfood gate for agent-spec itself; the
agent-spec repository still proves its own behavior through
`knowledge/requirements/req-requirements-compiler-plan-dag.md` and
`specs/task-requirements-compiler-plan-dag.spec.md`.

## Pipeline

Raw PRD:

- `docs/prd.md`

Compiled KLL requirements:

- `knowledge/requirements/req-note-create-create-note.md`
- `knowledge/requirements/req-note-list-list-notes.md`

Machine-readable compiler artifacts:

- `.agent-spec/requirements-plan.json`
- `.agent-spec/test_obligations.json`
- `.agent-spec/worktrees.json`
- `.agent-spec/questions.json`

Task contracts:

- `specs/task-req-note-create.spec.md`
- `specs/task-req-note-list.spec.md`

Rust proof:

- `src/lib.rs`
- `tests/noteapp_contract.rs`

## Commands

Run the Rust tests first:

```bash
cargo test --manifest-path fixtures/requirements-noteapp/Cargo.toml --quiet
```

Check that the Raw PRD still imports to the committed KLL requirement files:

```bash
cargo run --quiet -- requirements import --from fixtures/requirements-noteapp/docs/prd.md --out fixtures/requirements-noteapp/knowledge/requirements --check
```

This is the fixture's `requirements import --check` drift gate.

Validate the knowledge graph and plan:

```bash
cargo run --quiet -- lint-knowledge --knowledge fixtures/requirements-noteapp/knowledge --gate
cargo run --quiet -- requirements graph --knowledge fixtures/requirements-noteapp/knowledge --format json --gate
cargo run --quiet -- requirements plan --knowledge fixtures/requirements-noteapp/knowledge --specs fixtures/requirements-noteapp/specs --format json --gate
```

Regenerate the checked compiler artifacts:

```bash
cargo run --quiet -- requirements plan --knowledge fixtures/requirements-noteapp/knowledge --specs fixtures/requirements-noteapp/specs --format json --out fixtures/requirements-noteapp/.agent-spec/requirements-plan.json
cargo run --quiet -- requirements test-obligations --knowledge fixtures/requirements-noteapp/knowledge --specs fixtures/requirements-noteapp/specs --format json --out fixtures/requirements-noteapp/.agent-spec/test_obligations.json
cargo run --quiet -- requirements worktrees --knowledge fixtures/requirements-noteapp/knowledge --specs fixtures/requirements-noteapp/specs --base main --path-prefix ../agent-spec-worktrees --out fixtures/requirements-noteapp/.agent-spec/worktrees.json
cargo run --quiet -- requirements questions --knowledge fixtures/requirements-noteapp/knowledge --specs fixtures/requirements-noteapp/specs --format json --out fixtures/requirements-noteapp/.agent-spec/questions.json
```

Run lifecycle with requirement trace output:

```bash
cargo run --quiet -- lifecycle fixtures/requirements-noteapp/specs/task-req-note-create.spec.md --code fixtures/requirements-noteapp --run-log-dir fixtures/requirements-noteapp --format json
cargo run --quiet -- lifecycle fixtures/requirements-noteapp/specs/task-req-note-list.spec.md --code fixtures/requirements-noteapp --run-log-dir fixtures/requirements-noteapp --format json
```

Replay and visualize the requirement evidence:

```bash
cargo run --quiet -- requirements replay REQ-NOTE-CREATE --trace-dir fixtures/requirements-noteapp/.agent-spec/trace --format text
cargo run --quiet -- requirements trace-graph REQ-NOTE-CREATE --trace-dir fixtures/requirements-noteapp/.agent-spec/trace --format mermaid
```

## Governance step

Imported requirements start as `status: proposed` and stay informational until
a human accepts them:

```bash
agent-spec requirements transition REQ-NOTE-CREATE --to accepted --knowledge fixtures/requirements-noteapp/knowledge
agent-spec requirements transition REQ-NOTE-LIST --to accepted --knowledge fixtures/requirements-noteapp/knowledge
```

The checked-in fixture stores the accepted state so the plan/work-unit goldens
model the post-acceptance pipeline.
