#!/usr/bin/env bash
# Example: cadvm as the version / diff / VERIFY layer under an AI-CAD agent.
#
# It replays an "agent" editing a mesh part across iterations. cadvm pins each
# iteration, and `cadvm verify` GATES it: if the geometric change doesn't match
# what was asked, the bad iteration is reverted — exactly the loop an AI agent
# (or a CI gate) would run. No LLM and no Open CASCADE needed: the part is STL,
# so the diff is pure Rust.
#
# Run from anywhere:
#   examples/agent-loop.sh
set -u

# --- locate the repo and the cadvm binary -----------------------------------
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
if command -v cadvm >/dev/null 2>&1; then
  CADVM=cadvm
elif [ -x "$ROOT/target/release/cadvm" ]; then
  CADVM="$ROOT/target/release/cadvm"
elif [ -x "$ROOT/target/debug/cadvm" ]; then
  CADVM="$ROOT/target/debug/cadvm"
else
  echo "cadvm not found — run 'cargo build' or 'cargo install --path crates/cadvm-cli'." >&2
  exit 1
fi

V1="$ROOT/tests/fixtures/block_v1.stl"   # a bracket
V2="$ROOT/tests/fixtures/block_v2.stl"   # the bracket + a mounting boss

WORK="$(mktemp -d)"
cd "$WORK"
echo "Working in $WORK"
echo

# === 0. The agent's project is a cadvm repo =================================
"$CADVM" init >/dev/null
"$CADVM" config user.name "ai-agent" >/dev/null

# === 1. Baseline part ======================================================
cp "$V1" bracket.stl
"$CADVM" snapshot -m "baseline bracket" >/dev/null
echo "● baseline bracket committed"
echo

# === 2. Agent task: \"add a mounting boss\" ==================================
echo "▶ Agent iteration: add a mounting boss"
cp "$V2" bracket.stl                       # ← what the AI produced
"$CADVM" snapshot -m "agent: add mounting boss" >/dev/null

# The agent (or CI) GATES the result: the boss must add material.
echo "  verifying  added_tris > 0 …"
if "$CADVM" verify HEAD~1 HEAD --expect 'added_tris>0'; then
  echo "  ✓ accepted"
else
  echo "  ✗ rejected — reverting"; "$CADVM" revert HEAD >/dev/null
fi
echo

# === 3. Agent regression: it silently drops the boss =======================
echo "▶ Agent iteration: (buggy) regenerates and loses the boss"
cp "$V1" bracket.stl                       # ← oops, back to no boss
"$CADVM" snapshot -m "agent: regenerate (drops the boss)" >/dev/null

# Gate: the boss must NOT be removed. This catches the regression.
echo "  verifying  removed_tris < 20 …"
if "$CADVM" verify HEAD~1 HEAD --expect 'removed_tris<20'; then
  echo "  ✓ accepted"
else
  echo "  ✗ regression caught — reverting to the good version"
  "$CADVM" revert HEAD >/dev/null
fi
echo

# === 4. The compact machine-readable signal the agent consumes =============
# `verify --json` is the verification signal: pass/fail + the metrics behind it
# (small). `geom-diff --json` also exists but additionally carries the full mesh
# for the 3D viewer.
echo "▶ The JSON an agent parses (verify --json):"
"$CADVM" verify HEAD~1 HEAD --expect 'added_tris>0' --json
echo

echo "▶ History (the bad iteration was reverted, the boss survives):"
"$CADVM" log

rm -rf "$WORK"
