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
if ! command -v "$agent_command" >/dev/null 2>&1; then
  printf 'atlas-eval-agent-command: executable not found: %s\n' "$agent_command" >&2
  exit 2
fi
if ! command -v jq >/dev/null 2>&1; then
  printf '%s\n' 'atlas-eval-jq: jq is required by the opt-in runner' >&2
  exit 2
fi
if ! jq -e '.schema == "agent-spec/atlas-eval/run-plan-v1" and (.runs | type == "array" and length > 0)' "$plan" >/dev/null; then
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

"$agent_command" "$plan" "$@" >"$receipt_tmp"

mv -- "$receipt_tmp" "$receipts"
trap - EXIT
