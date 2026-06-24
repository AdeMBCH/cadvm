# cadvm for AI agents

If your stack **generates or edits CAD with AI** (text-to-CAD, parametric
copilots, design agents), cadvm is the **version / diff / verification layer**
underneath it. The AI produces geometry; cadvm pins every iteration, tells your
agent *what actually changed*, and lets it accept or roll back — automatically.

A human reviews a diff by eye. **An agent needs structured data** — that is what
`cadvm geom-diff --json` provides.

> ▶ **See it run:** the [Example: AI agent loop](example-agent.md) page replays
> this whole loop (accept a good edit, catch & revert a regression) with its real
> output — no LLM, no Open CASCADE.

## The agent loop

```bash
cadvm init                       # once, in the working directory

# …the AI writes/edits a CAD file (part.step, part.stl, …)…
cadvm snapshot -m "iteration 7"  # pin this AI iteration

# Machine-readable geometric diff vs the previous iteration:
cadvm geom-diff HEAD~1 HEAD --json
```

```jsonc
{
  "rev_a": "9c1f…", "rev_b": "a3b2…",
  "files": [{
    "path": "part.step",
    "kind": "brep",
    "diff": {
      "status": "ok",
      "added":   { "volume": 173.79, "faces": 252 },
      "removed": { "volume": 109.91, "faces": 179 },
      "common":  { "volume": 6266.98, "faces": 157 },
      "faces_topo": { "common": 6, "added": 3, "removed": 1 }
    }
  }]
}
```

Your agent parses that and decides — or it lets cadvm **judge** directly with
`verify`, which asserts expectations and returns pass/fail (exit code 0/1):

```bash
# "the edit should add material and remove almost none"
cadvm verify HEAD~1 HEAD --expect 'added_volume>50' --expect 'removed_volume<1' --json
# → {"report":{"pass":true,"checks":[...]}}   exit 0 = pass, 1 = fail
```

So the agent can:

- **verify / gate** — accept an iteration only if `cadvm verify` passes;
- **revert** — `cadvm revert HEAD` to undo a bad generation, then retry.

Available metrics: `added_volume`, `removed_volume`, `common_volume`,
`volume_delta`, `faces_added/removed/common` (STEP); `added_tris`,
`removed_tris`, `unchanged_tris`, `bbox_dx/dy/dz` (STL/OBJ).

Mesh files (STL/OBJ) emit the same shape with `unchanged` / `added` / `removed`
triangle layers — and need **no Open CASCADE** (pure-Rust diff).

## Evals & CI gates (no repository)

For evals and CI you usually have **two files** — the model's output and a
reference — and just want a geometric pass/fail. `--files` compares them
directly, **no repo, no snapshots**:

```bash
# Did the candidate match the reference closely enough?
cadvm verify --files candidate.stl reference.stl \
  --expect 'added_tris<5' --expect 'removed_tris<5'
echo $?    # 0 = pass (gate the model output), 1 = fail

cadvm geom-diff --files candidate.step reference.step --json   # raw signal
cadvm view     --files candidate.stl  reference.stl            # 3D diff for a human
```

This makes `cadvm verify` a drop-in **geometric assertion** for any eval harness,
RL reward, or CI job — exit code in, JSON out.

## Why it fits AI workflows

- **Structured feedback** — `--json` is a reward/verification signal for agents,
  eval harnesses and RL loops, not just a human-readable report.
- **Local-first & offline** — no cloud, no account; runs in CI or inside a
  sandbox next to the model.
- **Deterministic & cheap** — content-addressed storage dedupes the many
  near-identical iterations an agent produces.
- **Agent-friendly surface** — a plain CLI with JSON output, easy to wrap as a
  tool (e.g. an MCP server) the model calls.
- **Visual check for humans** — `cadvm view` renders the same diff in 3D when a
  person needs to look.

## Also useful for

- **Evals / benchmarks** for CAD-generating models — score "did the model
  produce the intended geometric change?".
- **Regression gates** in CI for generated or parametric CAD.

## Use it via MCP (no glue code)

cadvm ships an **MCP server** so an agent calls it as **native tools** — no
subprocess wiring or output parsing. Register it with your MCP client:

```jsonc
{
  "mcpServers": {
    "cadvm": { "command": "cadvm", "args": ["mcp"] }
  }
}
```

(With Claude Code: `claude mcp add cadvm -- cadvm mcp`.) It speaks JSON-RPC 2.0
over stdio — local, offline, no server to host.

The model then sees these tools:

| Tool | Does |
|------|------|
| `cadvm_status` | new / modified / deleted vs HEAD |
| `cadvm_snapshot` | pin an iteration (commit) |
| `cadvm_log` | history |
| `cadvm_diff` | metadata diff |
| `cadvm_geom_diff` | geometric diff (added/removed/common) |
| `cadvm_verify` | assert expectations → pass/fail |
| `cadvm_revert` | undo the last iteration |
| `cadvm_compare_files` | geometric diff of **two files** (no repo — for evals) |
| `cadvm_verify_files` | assert expectations on **two files** (no repo) → pass/fail |

Each tool takes an optional `repo` argument (the working directory); otherwise it
uses the server's current directory. So the loop above becomes a sequence of
tool calls the model makes on its own.
