# Documentation Review Checklist

Run this Lore-style pre-publish checklist for substantial agent-spec
documentation changes.

## Always

- [ ] The page has the correct doc type from `knowledge/standards/canon/doc-types.md`.
- [ ] The page follows the relevant canon and operational standards.
- [ ] The page has a rendered preview check and renders correctly, not only in source view.
- [ ] New or moved pages are reachable from a README, navigation entry, or linked task/spec.
- [ ] No internal-only hostnames, private tickets, private Slack references, or private account names appear in public-facing prose.

## Prose

- [ ] Voice and mood match the doc type.
- [ ] Claims are grounded in source, KLL, specs, tests, public references, or explicit assumptions.
- [ ] Code paths, commands, env vars, ids, and filenames use inline code formatting.
- [ ] Related KLL artifacts and Task Contracts are linked where traceability matters.

## Tools

- [ ] Run `bash scripts/docs-lint.sh` before review.
- [ ] Treat Harper, built-in Chinese docs lint, markdownlint, and lychee findings as pre-publish issues.
- [ ] If a local external tool is missing, record that in the review and rely on CI or a maintained environment before merge.
