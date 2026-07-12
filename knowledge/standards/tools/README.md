# Documentation Tools

agent-spec adopts Lore's documentation gate structure and adapts the toolchain
for bilingual documentation:

| Tool | Purpose |
| --- | --- |
| Harper | English prose linting for spelling, grammar, and basic prose quality. |
| agent-spec Chinese docs lint | Built-in deterministic checks for Chinese documentation hygiene. |
| markdownlint | Markdown structure: headings, fence tags, lists, and table mechanics. |
| lychee | Link integrity for internal and external links. |

Run:

```bash
bash scripts/docs-lint.sh
```

The script should run every installed tool, continue after findings so all
results are visible, and exit non-zero if a tool reports findings. Missing
external tools should warn but still run the tools that exist. Set
`DOCS_LINT_REQUIRE_EXTERNAL=1` when at least one external tool must be present,
or `DOCS_LINT_REQUIRE_EXTERNAL=all` in CI to require Harper, markdownlint, and
lychee to all execute.

The built-in Chinese docs lint always runs. Its initial project rules are:

- `zh-no-fullwidth-space`: reject full-width spaces in Markdown prose.
- `zh-no-replacement-char`: reject Unicode replacement characters that usually
  indicate encoding damage.
- `zh-no-unresolved-placeholder`: reject unresolved placeholders such as
  `TODO`, `TBD`, `待定`, `这里填写`, and lorem text outside template files.
