# CI templates

Workflow files meant to be copied into **your** repository, not run by this
one. They live here rather than under `.github/workflows/` so they do not
execute on every agent-spec pull request — two workflows with the same `name:`
racing for the same runner pool produced intermittent phantom red checks.

| Template | What it does | Copy to |
|---|---|---|
| `contract-guard-minimal.yml` | Runs `agent-spec guard` and posts the result — the fastest possible setup | `.github/workflows/contract-guard.yml` |

For the full version — spec lint with a score threshold, verification against
the PR's changed files, and a rendered Contract review summary posted to the
PR — copy this repository's own
[`.github/workflows/contract-guard.yml`](../../.github/workflows/contract-guard.yml).
It is both agent-spec's live gate and a template, so what you copy is what is
actually being run here.

Pick the minimal one when you want a green/red signal with no setup, and the
full one when you want the review summary in the PR conversation.
