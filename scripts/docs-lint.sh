#!/usr/bin/env bash
set -u

status=0
ran_external=0
ran_internal=0
ran_harper=0
ran_markdownlint=0
ran_lychee=0
missing_external=()

doc_paths=(README.md AGENTS.md docs knowledge/standards)

run_tool() {
  local name="$1"
  shift
  if command -v "$name" >/dev/null 2>&1; then
    ran_external=$((ran_external + 1))
    case "$name" in
      markdownlint-cli2) ran_markdownlint=1 ;;
      lychee) ran_lychee=1 ;;
    esac
    printf '\n== %s ==\n' "$name"
    "$@" || status=1
  else
    missing_external+=("$name")
    printf 'warning: %s not installed; skipping\n' "$name" >&2
  fi
}

# Harper findings fail the run unless DOCS_LINT_HARPER_ADVISORY=1. The docs are
# bilingual and full of project vocabulary, so English prose lint is advisory in
# CI: it must run and print, but only markdownlint/lychee/zh-lint gate.
harper_status() {
  if [ "${DOCS_LINT_HARPER_ADVISORY:-0}" = "1" ]; then
    printf 'note: harper findings are advisory (DOCS_LINT_HARPER_ADVISORY=1)\n'
  else
    status=1
  fi
}

run_harper() {
  # Prose lint only reads Markdown; feeding HTML/JS produces token noise.
  local md_files=()
  while IFS= read -r -d '' file; do
    md_files+=("$file")
  done < <(find "${doc_paths[@]}" -type f -name '*.md' -print0)

  if [ -n "${HARPER_CMD:-}" ]; then
    ran_external=$((ran_external + 1))
    ran_harper=1
    printf '\n== harper ==\n'
    # shellcheck disable=SC2086
    $HARPER_CMD "${md_files[@]}" || harper_status
  elif command -v harper-cli >/dev/null 2>&1; then
    ran_external=$((ran_external + 1))
    ran_harper=1
    printf '\n== harper-cli ==\n'
    harper-cli lint "${md_files[@]}" || harper_status
  elif command -v harper >/dev/null 2>&1; then
    ran_external=$((ran_external + 1))
    ran_harper=1
    printf '\n== harper ==\n'
    harper lint "${md_files[@]}" || harper_status
  else
    missing_external+=("harper-cli")
    printf 'warning: harper-cli or harper not installed; skipping English prose lint\n' >&2
  fi
}

run_chinese_lint() {
  ran_internal=$((ran_internal + 1))
  printf '\n== agent-spec Chinese docs lint ==\n'

  local found=0
  local fullwidth_space
  local replacement_char
  fullwidth_space="$(printf '\343\200\200')"
  replacement_char="$(printf '\357\277\275')"

  while IFS= read -r -d '' file; do
    case "$file" in
      *-template.md) continue ;;
    esac

    if grep -n "$fullwidth_space" "$file" >/tmp/agent-spec-zhlint-match.$$; then
      sed "s#^#$file:#" /tmp/agent-spec-zhlint-match.$$
      printf '  rule: zh-no-fullwidth-space\n'
      found=1
    fi

    if grep -n "$replacement_char" "$file" >/tmp/agent-spec-zhlint-match.$$; then
      sed "s#^#$file:#" /tmp/agent-spec-zhlint-match.$$
      printf '  rule: zh-no-replacement-char\n'
      found=1
    fi

    if grep -nE 'TODO|TBD|待定|这里填写|lorem|Lorem' "$file" \
      | grep -vE '`[^`]*(TODO|TBD|待定|这里填写|lorem|Lorem)[^`]*`' \
      >/tmp/agent-spec-zhlint-match.$$; then
      sed "s#^#$file:#" /tmp/agent-spec-zhlint-match.$$
      printf '  rule: zh-no-unresolved-placeholder\n'
      found=1
    fi
  done < <(find "${doc_paths[@]}" -type f -name '*.md' -print0)

  rm -f /tmp/agent-spec-zhlint-match.$$

  if [ "$found" -eq 0 ]; then
    printf 'ok: Chinese docs lint passed\n'
  else
    status=1
  fi
}

run_chinese_lint
run_harper
run_tool markdownlint-cli2 markdownlint-cli2 "README.md" "AGENTS.md" "docs/**/*.md" "knowledge/standards/**/*.md"
run_tool lychee lychee README.md AGENTS.md docs/ knowledge/standards/

if [ "${DOCS_LINT_REQUIRE_EXTERNAL:-0}" = "all" ]; then
  required_missing=()
  [ "$ran_harper" -eq 1 ] || required_missing+=("harper-cli")
  [ "$ran_markdownlint" -eq 1 ] || required_missing+=("markdownlint-cli2")
  [ "$ran_lychee" -eq 1 ] || required_missing+=("lychee")
  if [ "${#required_missing[@]}" -gt 0 ]; then
    printf 'error: DOCS_LINT_REQUIRE_EXTERNAL=all requires Harper, markdownlint-cli2, and lychee; missing: %s\n' "${required_missing[*]}" >&2
    exit 2
  fi
elif [ "$ran_external" -eq 0 ]; then
  printf 'warning: no external documentation lint tools were installed\n' >&2
  if [ "${DOCS_LINT_REQUIRE_EXTERNAL:-0}" = "1" ]; then
    printf 'error: DOCS_LINT_REQUIRE_EXTERNAL=1 requires at least one external docs lint tool\n' >&2
    exit 2
  fi
fi

exit "$status"
