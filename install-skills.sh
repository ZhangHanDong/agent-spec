#!/usr/bin/env bash
set -euo pipefail

SKILL_DIR="${HOME}/.claude/skills"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

echo "=== agent-spec skills installer ==="
echo

# Step 1: Install CLI
if command -v agent-spec &>/dev/null; then
  CURRENT=$(agent-spec --version 2>/dev/null || echo "unknown")
  echo "[ok] agent-spec CLI already installed: ${CURRENT}"
else
  echo "[..] Installing agent-spec CLI via cargo..."
  if command -v cargo &>/dev/null; then
    cargo install agent-spec
    echo "[ok] agent-spec CLI installed"
  else
    echo "[!!] cargo not found. Install Rust first: https://rustup.rs"
    echo "     Then run: cargo install agent-spec"
    exit 1
  fi
fi

echo

# Step 2: Install skills
mkdir -p "${SKILL_DIR}"

for skill in agent-spec-tool-first agent-spec-authoring agent-spec-estimate agent-spec-intent-compiler agent-spec-wiki; do
  SRC="${SCRIPT_DIR}/skills/${skill}"
  DST="${SKILL_DIR}/${skill}"

  if [ ! -d "${SRC}" ]; then
    echo "[skip] ${skill} — not found in ${SCRIPT_DIR}/skills/"
    continue
  fi

  # Copy (overwrite) to ensure latest version
  rm -rf "${DST}"
  cp -r "${SRC}" "${DST}"
  echo "[ok] ${skill} → ${DST}"
done

echo
echo "Done. All agent-spec skills are ready for Claude Code."
echo "Verify with: ls ~/.claude/skills/agent-spec-*"
