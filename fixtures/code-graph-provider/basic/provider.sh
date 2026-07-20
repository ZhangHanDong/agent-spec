#!/bin/sh

IFS= read -r request

emit_payload() {
  state="$1"
  worktree="$2"
  order="$3"
  schema="${4:-agent-spec/code-graph-provider/extraction-payload-v1}"
  affected='[]'
  diagnostics='[]'
  if [ "$state" = "partial" ]; then
    affected='["src/broken.fixture"]'
    diagnostics='[{"code":"fixture-parse","severity":"warning","message":"partial fixture parse","path":"src/broken.fixture"}]'
  elif [ "$state" = "stale" ]; then
    affected='["src/lib.fixture"]'
    diagnostics='[{"code":"fixture-stale","severity":"warning","message":"fixture source changed","path":"src/lib.fixture"}]'
  fi
  module='{"id":"fixture-extractor:module:root","name":"root","kind":"module","path":"src/lib.fixture","span":null,"provenance":{"extractor":"fixture-parser","extractor_version":"1.0.0","evidence":"fixture syntax","confidence":"exact"}}'
  function='{"id":"fixture-extractor:function:root/run","name":"run","kind":"function","path":"src/lib.fixture","span":{"line_start":2,"column_start":1,"line_end":3,"column_end":2},"provenance":{"extractor":"fixture-parser","extractor_version":"1.0.0","evidence":"fixture syntax","confidence":"exact"}}'
  if [ "$order" = "reverse" ]; then
    nodes="[$module,$function]"
  else
    nodes="[$function,$module]"
  fi
  printf '{"schema":"%s","provider_id":"fixture-extractor","provider_version":"1.0.0","language":"fixture","worktree_id":"%s","freshness":{"state":"%s","inputs":[{"path":"src/lib.fixture","fingerprint":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}],"affected_paths":%s},"nodes":%s,"edges":[{"from":"fixture-extractor:module:root","to":"fixture-extractor:function:root/run","kind":"contains","provenance":{"extractor":"fixture-parser","extractor_version":"1.0.0","evidence":"fixture syntax","confidence":"exact"}}],"diagnostics":%s}\n' "$schema" "$worktree" "$state" "$affected" "$nodes" "$diagnostics"
}

case "$request" in
  *'"conformance_case":"fresh"'*) emit_payload fresh fixture-worktree forward ;;
  *'"conformance_case":"repeat"'*) emit_payload fresh fixture-worktree reverse ;;
  *'"conformance_case":"partial-parse"'*) emit_payload partial fixture-worktree forward ;;
  *'"conformance_case":"stale"'*) emit_payload stale fixture-worktree forward ;;
  *'"conformance_case":"wrong-worktree"'*) emit_payload fresh another-worktree forward ;;
  *'"conformance_case":"unknown-schema"'*) emit_payload fresh fixture-worktree forward agent-spec/code-graph-provider/extraction-payload-v99 ;;
  *'"conformance_case":"bounded-output"'*)
    i=0
    while [ "$i" -lt 5000 ]; do
      printf x
      i=$((i + 1))
    done
    ;;
  *'"conformance_case":"cancellation"'*) while :; do :; done ;;
  *)
    printf 'fixture-unknown-case\n' >&2
    exit 17
    ;;
esac
