# Limits & roadmap

## Current limits

- The geometric diff reports **volumes** (added/removed/common) and a heuristic
  **face-to-face** classification; it does not yet do exact topological face
  correspondence.
- cadvm cannot merge two concurrent edits of the same STEP file.
- `geom-diff` / `view` require the `cadvm-geom` helper (OCCT). The rest of cadvm
  works without it.
- Built and tested on Linux only so far (see [Platform support](platforms.md)).

## Done

- **VCS core** — snapshots, log, status, diff, branches, switch, revert,
  checkout, gc, config/author, deduplicated chunk-only storage.
- **Geometric diff** — `cadvm-geom` (C++/OCCT): boolean volumes + metrics +
  heuristic topological face diff.
- **3D viewer** — `cadvm view`: self-contained WebGL HTML, full-part context +
  green/red/grey changes.
- **Interactive TUI** — `cadvm ui`.
- **Shell completions** — `cadvm completions`.

## Next

- Exact topological face correspondence (not just volumetric / heuristic).
- Multi-OS CI and prebuilt binaries for easy installation.
- A staging index and richer merge tooling.
- Hosting this documentation (e.g. GitHub Pages).
