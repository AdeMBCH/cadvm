<p align="center">
  <img src="cadvm-logo.png" alt="cadvm" width="420">
</p>

# Introduction

**cadvm** (CAD Version Manager) is a **local-first version control system for CAD
files** — **STEP/STP** (B-Rep) and **STL/OBJ** (triangle mesh). It brings
Git-like workflows — snapshots, branches, diff, checkout, revert — to CAD data,
and goes further with a **geometric diff** and a **3D viewer** that show what
actually changed in the geometry, not just in the bytes.

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

- stores versions **deduplicated** (content-addressed, gzip-compressed chunks);
- summarizes each file with **light metadata** (STEP schema/entities, or mesh
  triangle/vertex counts);
- computes a real **geometric diff** (added / removed / common) — via Open CASCADE
  for STEP/STP, and a pure-Rust triangle diff for STL/OBJ;
- renders that diff in a **self-contained 3D HTML viewer** (no server, no cloud).

> 🧊 **See it live:** the [Live demo](demo.md) embeds a real diff you can rotate
> and zoom right in your browser.

## Architecture

cadvm is built in three layers:

| Layer | What | Tech |
|-------|------|------|
| **VCS core** | commits, manifests, refs, branches, status, diff, checkout, storage; STL/OBJ mesh diff | 100% Rust |
| **Geometry helper** | STEP/STP added/removed/common volumes + face diff, tessellation | C++ / Open CASCADE (subprocess) |
| **Viewer** | interactive 3D diff | self-contained WebGL HTML |

The Rust core never links Open CASCADE directly — it calls the `cadvm-geom`
helper **as a subprocess**, keeping the heavy CAD dependency isolated. Open
CASCADE is needed only for the **STEP/STP** geometric diff; the VCS, the TUI and
the **STL/OBJ** mesh diff all work without it.

## Crates

- `cadvm-store` — content-addressed storage (BLAKE3, blobs, chunks).
- `cadvm-core` — repository logic.
- `cadvm-cli` — the `cadvm` binary (CLI + TUI).
- `cpp/cadvm-geom` — the C++/OCCT geometry helper.

The **[API reference (rustdoc)](api/cadvm_core/index.html)** documents these
crates.

Continue with [Installation](installation.md).
