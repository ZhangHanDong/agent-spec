#!/usr/bin/env bash
if [[ -z "${ATLAS_EVAL_SERVING_COMMAND:-}" ]]; then
  printf '%s\n' 'atlas-serving-ab-command: set ATLAS_EVAL_SERVING_COMMAND explicitly' >&2
  exit 2
fi

set -euo pipefail

if [[ $# -lt 2 ]]; then
  printf '%s\n' 'usage: run-serving-ab-opt-in.sh PLAN RECEIPTS [-- DRIVER_ARG...]' >&2
  exit 2
fi

plan=$1
receipts=$2
shift 2
if [[ ${1:-} == '--' ]]; then
  shift
fi

serving_command=$ATLAS_EVAL_SERVING_COMMAND
if [[ $serving_command == *$'\n'* || $serving_command == *$'\r'* ]]; then
  printf '%s\n' 'atlas-serving-ab-command: command must be one executable path or name' >&2
  exit 2
fi
resolved_command=$(command -v -- "$serving_command" 2>/dev/null || true)
if [[ $resolved_command != */* || ! -f $resolved_command || ! -x $resolved_command ]]; then
  printf 'atlas-serving-ab-command: executable not found: %s\n' "$serving_command" >&2
  exit 2
fi
if ! command -v jq >/dev/null 2>&1; then
  printf '%s\n' 'atlas-serving-ab-jq: jq is required by the opt-in runner' >&2
  exit 2
fi
if ! jq -e '
  .schema == "agent-spec/atlas-eval/serving-plan-v1"
  and (.experiment_version | type == "string" and length > 0)
  and (.runs | type == "array" and length > 0)
  and ([.runs[].arm] | unique == ["direct", "worker"])
  and ([.runs[].profile] | unique == ["light", "mixed", "source-heavy", "traversal"])
  and all(.runs[];
    (.run_id | type == "string" and length == 64)
    and (.trial | type == "number" and . >= 1 and floor == .)
    and (.burst_width | type == "number" and . >= 2 and floor == .))
' "$plan" >/dev/null; then
  printf '%s\n' 'atlas-serving-ab-plan: malformed or empty serving plan' >&2
  exit 2
fi

receipt_dir=$(dirname -- "$receipts")
receipt_name=$(basename -- "$receipts")
if [[ ! -d $receipt_dir ]]; then
  printf 'atlas-serving-ab-output: parent directory does not exist: %s\n' "$receipt_dir" >&2
  exit 2
fi
receipt_tmp=$(mktemp "$receipt_dir/.${receipt_name}.tmp.XXXXXX")
cleanup() {
  rm -f -- "$receipt_tmp"
}
trap cleanup EXIT

"$resolved_command" "$plan" "$@" >"$receipt_tmp"
if ! jq -e '
  .schema == "agent-spec/atlas-eval/serving-receipts-v1"
  and (.experiment_version | type == "string" and length > 0)
  and (.plan_fingerprint | type == "string" and length == 64)
  and (.runs | type == "array" and length > 0)
' "$receipt_tmp" >/dev/null; then
  printf '%s\n' 'atlas-serving-ab-receipt: driver emitted an invalid receipt bundle' >&2
  exit 2
fi

mv -- "$receipt_tmp" "$receipts"
trap - EXIT

