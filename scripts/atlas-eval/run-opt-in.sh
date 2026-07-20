#!/usr/bin/env bash
if [[ -z "${ATLAS_EVAL_AGENT_COMMAND:-}" ]]; then
  printf '%s\n' 'atlas-eval-agent-command: set ATLAS_EVAL_AGENT_COMMAND explicitly' >&2
  exit 2
fi

set -euo pipefail

if [[ $# -lt 2 ]]; then
  printf '%s\n' 'usage: run-opt-in.sh PLAN RECEIPTS [-- AGENT_ARG...]' >&2
  exit 2
fi

plan=$1
receipts=$2
shift 2
if [[ ${1:-} == '--' ]]; then
  shift
fi

agent_command=$ATLAS_EVAL_AGENT_COMMAND
if [[ $agent_command == *$'\n'* || $agent_command == *$'\r'* ]]; then
  printf '%s\n' 'atlas-eval-agent-command: command must be one executable path or name' >&2
  exit 2
fi
resolved_command=$(command -v -- "$agent_command" 2>/dev/null || true)
if [[ $resolved_command != */* || ! -f $resolved_command || ! -x $resolved_command ]]; then
  printf 'atlas-eval-agent-command: executable not found: %s\n' "$agent_command" >&2
  exit 2
fi
if ! command -v jq >/dev/null 2>&1; then
  printf '%s\n' 'atlas-eval-jq: jq is required by the opt-in runner' >&2
  exit 2
fi
if ! jq -e '
  def nonempty_string: type == "string" and length > 0;
  def valid_run:
    if type != "object" then
      false
    else
      ((keys | sort) == ([
        "arm",
        "cache_condition",
        "case_id",
        "model",
        "permissions",
        "prompt",
        "repository",
        "revision",
        "trial"
      ] | sort))
      and (.case_id | nonempty_string)
      and (.model | nonempty_string)
      and (.prompt | nonempty_string)
      and (.repository | nonempty_string)
      and (.revision | nonempty_string)
      and (.arm == "atlas" or .arm == "baseline")
      and (.permissions == "read-only" or .permissions == "workspace-write")
      and (.cache_condition == "cold" or .cache_condition == "warm")
      and (.trial | type == "number" and . > 0 and floor == .)
    end;
  .schema == "agent-spec/atlas-eval/run-plan-v1"
  and (.runs | type == "array" and length > 0)
  and all(.runs[]; valid_run)
' "$plan" >/dev/null; then
  printf '%s\n' 'atlas-eval-plan: malformed or empty run plan' >&2
  exit 2
fi

receipt_dir=$(dirname -- "$receipts")
receipt_name=$(basename -- "$receipts")
receipt_tmp=$(mktemp "$receipt_dir/.${receipt_name}.tmp.XXXXXX")
cleanup() {
  rm -f -- "$receipt_tmp"
}
trap cleanup EXIT

# Keep agent stdout opaque; versioned query metrics and legacy coverage belong to summarize.
"$resolved_command" "$plan" "$@" >"$receipt_tmp"

mv -- "$receipt_tmp" "$receipts"
trap - EXIT
