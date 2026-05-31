#!/usr/bin/env bash
set -euo pipefail

allow_placeholders=false
if [[ "${1:-}" == "--allow-placeholders" ]]; then
  allow_placeholders=true
fi

root="$(cd "$(dirname "$0")/.." && pwd)"
src="$root/src"
book_toml="$root/book.toml"

fail=0

if ! grep -q 'mermaid.min.js' "$book_toml" || ! grep -q 'mermaid-init.js' "$book_toml"; then
  echo "FAIL: book.toml is missing Mermaid HTML assets"
  fail=1
fi

if [[ ! -f "$root/mermaid.min.js" || ! -f "$root/mermaid-init.js" ]]; then
  echo "FAIL: Mermaid runtime files are missing"
  fail=1
fi

if ! grep -q 'language-mermaid' "$root/mermaid-init.js" || ! grep -Eq 'mermaid\.(run|init)' "$root/mermaid-init.js"; then
  echo "FAIL: mermaid-init.js does not convert and render Mermaid code blocks"
  fail=1
fi

if ! grep -Rqs '^```mermaid' "$src"; then
  echo "FAIL: source has no Mermaid smoke diagram"
  fail=1
fi

count_table_blocks() {
  awk '
    BEGIN { in_table = 0; count = 0 }
    /^[[:space:]]*\|/ {
      if (!in_table) { count++; in_table = 1 }
      next
    }
    { in_table = 0 }
    END { print count }
  ' "$1"
}

count_code_blocks() {
  awk '
    /^```/ {
      fence++
      if ($0 ~ /^```(rust|toml|spec|bash|shell|text|markdown|md)$/) { useful++ }
    }
    END { print useful }
  ' "$1"
}

check_chapter() {
  local file="$1"
  local min_chars="$2"
  local max_chars="$3"
  local min_visuals="$4"
  local max_visuals="$5"
  local min_mermaid="$6"

  local path="$src/$file"
  if [[ ! -f "$path" ]]; then
    echo "FAIL: missing chapter file $file"
    fail=1
    return
  fi

  if $allow_placeholders && grep -q '本章正文待编写' "$path"; then
    echo "SKIP: $file is still a placeholder"
    return
  fi

  local chars mermaid images tables code visuals
  chars="$(wc -m < "$path" | tr -d ' ')"
  mermaid="$(grep -c '^```mermaid' "$path" || true)"
  images="$(grep -c '!\[[^]]*\](' "$path" || true)"
  tables="$(count_table_blocks "$path")"
  code="$(count_code_blocks "$path")"
  visuals=$((mermaid + images + tables + code))

  echo "$file: chars=$chars visuals=$visuals mermaid=$mermaid"

  if (( chars < min_chars || chars > max_chars )); then
    echo "FAIL: $file characters $chars outside budget $min_chars-$max_chars"
    fail=1
  fi
  if (( visuals < min_visuals || visuals > max_visuals )); then
    echo "FAIL: $file visual units $visuals outside budget $min_visuals-$max_visuals"
    fail=1
  fi
  if (( mermaid < min_mermaid )); then
    echo "FAIL: $file mermaid diagrams $mermaid below floor $min_mermaid"
    fail=1
  fi
}

check_chapter ch01-environment-bottleneck.md 4000 5000 3 4 1
check_chapter ch02-harness-engineering.md 6000 7000 5 6 2
check_chapter ch03-spec-driven-development.md 6000 7000 5 6 1
check_chapter ch04-bdd-spine.md 6000 7000 5 6 2
check_chapter ch05-agent-spec-standards.md 5000 6000 4 5 1
check_chapter ch06-agent-spec-verdict.md 6000 7000 5 6 2
check_chapter ch07-codex-rs-rust-contracts.md 6000 7000 4 5 1
check_chapter ch08-rust-agent-project.md 7000 8000 6 7 2
check_chapter ch09-spec-driven-writing.md 5000 6000 4 5 1

exit "$fail"
