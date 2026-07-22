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

d4_mode=${ATLAS_EVAL_D4_MODE:-}
if [[ $d4_mode != direct && $d4_mode != worker ]]; then
  printf '%s\n' 'atlas-eval-d4-mode: set ATLAS_EVAL_D4_MODE to direct or worker' >&2
  exit 2
fi
d4_receipt=${ATLAS_EVAL_D4_RECEIPT:-}
if [[ -z $d4_receipt || ! -f $d4_receipt ]]; then
  printf '%s\n' 'atlas-eval-d4-receipt: set ATLAS_EVAL_D4_RECEIPT to a readable file' >&2
  exit 2
fi
if command -v sha256sum >/dev/null 2>&1; then
  d4_hash=$(sha256sum -- "$d4_receipt" | awk '{print $1}')
elif command -v shasum >/dev/null 2>&1; then
  d4_hash=$(shasum -a 256 -- "$d4_receipt" | awk '{print $1}')
else
  printf '%s\n' 'atlas-eval-d4-hash: sha256sum or shasum is required' >&2
  exit 2
fi
export ATLAS_EVAL_SERVING_MODE=$d4_mode
export ATLAS_EVAL_CONCURRENT_QUERY_RECEIPT_PATH=$d4_receipt
export ATLAS_EVAL_CONCURRENT_QUERY_RECEIPT_HASH=$d4_hash

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
