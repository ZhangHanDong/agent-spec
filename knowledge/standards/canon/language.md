# Language Standard

This standard adapts Lore's language canon for agent-spec documentation.

## Voice

- Prefer direct, active voice.
- Use imperative mood for procedural steps.
- Use indicative mood for reference and explanation.
- Avoid unsupported claims; ground claims in code paths, KLL artifacts, specs,
  public references, or the document itself.
- Avoid AI theater: do not claim a model verified behavior unless lifecycle,
  tests, trace, or human review evidence exists.

## Terms

- Use `agent-spec` for the tool name.
- Use `Task Contract` for `.spec.md` task files.
- Use `KLL` only after first spelling out Knowledge Liveness Layer when the
  audience may be new.
- Use `requirement replay` for evidence replay, not deterministic LLM replay.
