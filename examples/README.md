# Examples

## `agent-loop.sh` — cadvm under an AI-CAD agent

A runnable, self-contained demo of the loop an AI-CAD pipeline runs with cadvm as
its **version / diff / verify** layer:

```text
agent edits the part → cadvm snapshot → cadvm verify (gate) → revert if it fails
```

It replays an "agent" working on a bracket:

1. **baseline** bracket is committed;
2. the agent **adds a mounting boss** → `cadvm verify --expect 'added_tris>0'`
   passes → the iteration is **accepted**;
3. a buggy iteration **silently drops the boss** →
   `cadvm verify --expect 'removed_tris<20'` fails → the regression is
   **caught and reverted**.

No LLM and no Open CASCADE required — the part is an STL, so the geometric diff
is pure Rust.

```bash
# build or install cadvm first, then:
examples/agent-loop.sh
```

The decision is driven by `cadvm verify`'s **exit code** (0 = pass, non-zero =
fail) — the same hook an agent or a CI gate would use. See
[cadvm for AI](https://adembch.github.io/cadvm/ai.html) for the MCP-tool version
of the same loop.
