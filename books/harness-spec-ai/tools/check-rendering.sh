#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
html="$root/book/index.html"
fail=0

if [[ ! -f "$html" ]]; then
  echo "FAIL: missing rendered HTML at $html"
  echo "Run: mdbook build books/harness-spec-ai"
  exit 1
fi

if [[ ! -f "$root/book/mermaid.min.js" || ! -f "$root/book/mermaid-init.js" ]]; then
  echo "FAIL: rendered book is missing Mermaid JavaScript assets"
  fail=1
fi

if ! grep -q 'mermaid.min.js' "$html" || ! grep -q 'mermaid-init.js' "$html"; then
  echo "FAIL: rendered HTML does not load Mermaid JavaScript assets"
  fail=1
fi

if ! grep -Eq 'language-mermaid|class="mermaid"' "$html"; then
  echo "FAIL: rendered HTML does not contain a Mermaid diagram source"
  fail=1
fi

if ! grep -q 'mdbookMermaid' "$root/book/mermaid-init.js"; then
  echo "FAIL: rendered Mermaid init script does not include runtime conversion logic"
  fail=1
fi

if (( fail == 0 )); then
  echo "OK: mdBook output loads Mermaid assets and contains a Mermaid diagram source"
fi

exit "$fail"
