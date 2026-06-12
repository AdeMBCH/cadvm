<p align="center">
  <img src="cadvm-logo.png" alt="cadvm" width="420">
</p>

# Introduction

**cadvm** (CAD Version Manager) is a **local-first version control system for
STEP/STP CAD files**. It brings Git-like workflows — snapshots, branches, diff,
checkout, revert — to CAD data, and goes further with a **geometric diff** and a
**3D viewer** that show what actually changed in the geometry, not just in the
bytes.

```text
cadvm init
cadvm snapshot -m "Cube with a Ø5 hole"
# …edit the part…
cadvm snapshot -m "Ø5 → Ø8"
cadvm geom-diff HEAD~1 HEAD     # volumes added / removed / common
cadvm view HEAD~1 HEAD          # interactive 3D diff in your browser
cadvm ui                        # full-screen terminal dashboard
```

## Why cadvm?

CAD files are awful to version with Git: a tiny geometric edit re-exports the
whole STEP file with renumbered entities and a new timestamp, so a textual diff
is pure noise. cadvm instead:

- stores versions **deduplicated** (content-addressed blobs + chunks);
- summarizes each file with **light STEP metadata** (schema, entity counts);
- computes a real **geometric diff** (added / removed / common material) using
  Open CASCADE;
- renders that diff in a **self-contained 3D HTML viewer** (no server, no cloud).

## Architecture

cadvm is built in three layers:

| Layer | What | Tech |
|-------|------|------|
| **VCS core** | commits, manifests, refs, branches, status, diff, checkout, storage | 100% Rust |
| **Geometry helper** | added/removed/common volumes + face diff, tessellation | C++ / Open CASCADE (subprocess) |
| **Viewer** | interactive 3D diff | self-contained WebGL HTML |

The Rust core never links Open CASCADE directly — it calls the `cadvm-geom`
helper **as a subprocess**, keeping the heavy CAD dependency isolated. The whole
VCS works without it; only `geom-diff` and `view` need it.

## Crates

- `cadvm-store` — content-addressed storage (BLAKE3, blobs, chunks).
- `cadvm-core` — repository logic.
- `cadvm-cli` — the `cadvm` binary (CLI + TUI).
- `cpp/cadvm-geom` — the C++/OCCT geometry helper.

Continue with [Installation](installation.md).
