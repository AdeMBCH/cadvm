# cadvm for AI agents

If your stack **generates or edits CAD with AI** (text-to-CAD, parametric
copilots, design agents), cadvm is the **version / diff / verification layer**
underneath it. The AI produces geometry; cadvm pins every iteration, tells your
agent *what actually changed*, and lets it accept or roll back — automatically.

A human reviews a diff by eye. **An agent needs structured data** — that is what
`cadvm geom-diff --json` provides.

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

> `cadvm verify` is built in (above). An **MCP server** — exposing snapshot /
> diff / geom-diff / verify as tools an agent calls natively — is next on the
> [roadmap](roadmap.md).
