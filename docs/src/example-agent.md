# Example: AI agent loop

A runnable, self-contained demo of cadvm as the **version / diff / verify** layer
under an AI-CAD agent — accept a good edit, **catch and revert a regression**. No
LLM and no Open CASCADE: the part is an STL, so the geometric diff is pure Rust.

Script: [`examples/agent-loop.sh`](https://github.com/AdeMBCH/cadvm/blob/main/examples/agent-loop.sh).

```bash
examples/agent-loop.sh    # after building or installing cadvm
```

## What it runs

```bash
cadvm init
cadvm snapshot -m "baseline bracket"

# Agent adds a mounting boss → GATE: it must add material
cp block_v2.stl bracket.stl
cadvm snapshot -m "agent: add mounting boss"
cadvm verify HEAD~1 HEAD --expect 'added_tris>0'   # exit 0 = accept

# Agent regresses and drops the boss → GATE: nothing significant removed
cp block_v1.stl bracket.stl
cadvm snapshot -m "agent: regenerate (drops the boss)"
cadvm verify HEAD~1 HEAD --expect 'removed_tris<20' \
  || cadvm revert HEAD                              # exit 1 = revert
```

The accept/revert decision is driven by **`cadvm verify`'s exit code** — exactly
the hook an agent or a CI gate uses.

## What it prints

```text
● baseline bracket committed

▶ Agent iteration: add a mounting boss
  verifying  added_tris > 0 …
  ✓ added_tris > 0   (actual 158)
  PASS (1 checks)
  ✓ accepted

▶ Agent iteration: (buggy) regenerates and loses the boss
  verifying  removed_tris < 20 …
  ✗ removed_tris < 20   (actual 158)
  FAIL (1/1 checks failed)
  ✗ regression caught — reverting to the good version

▶ The JSON an agent parses (verify --json):
{
  "file": "bracket.stl",
  "report": {
    "pass": true,
    "metrics": { "added_tris": 158, "removed_tris": 64, "unchanged_tris": 114,
                 "bbox_dx": 40, "bbox_dy": 30, "bbox_dz": 28 },
    "checks": [ { "metric": "added_tris", "op": "Gt", "expected": 0,
                  "actual": 158, "pass": true } ]
  }
}

▶ History (the bad iteration was reverted, the boss survives):
commit …  Revert "agent: regenerate (drops the boss)"
commit …  agent: regenerate (drops the boss)
commit …  agent: add mounting boss
commit …  baseline bracket
```

## In an agent (via MCP)

The same loop runs as native tool calls — `cadvm_snapshot`, `cadvm_verify`,
`cadvm_revert` — when cadvm is wired in as an [MCP server](ai.md#use-it-via-mcp-no-glue-code).
