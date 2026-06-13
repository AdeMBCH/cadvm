# Limits & roadmap

## Current limits

- The geometric diff reports **volumes** (added/removed/common) and a heuristic
  **face-to-face** classification; it does not yet do exact topological face
  correspondence.
- cadvm cannot merge two concurrent edits of the same STEP file.
- `geom-diff` / `view` require the `cadvm-geom` helper (OCCT). The rest of cadvm
  works without it.

## Done

- **VCS core** — snapshots, log, status, diff, branches, switch, revert,
  checkout, gc, config/author, deduplicated, gzip-compressed chunk storage.
- **Geometric diff** — `cadvm-geom` (C++/OCCT): boolean volumes + metrics +
  heuristic topological face diff.
- **3D viewer** — `cadvm view`: self-contained WebGL HTML, full-part context +
  green/red/grey changes.
- **Interactive TUI** — `cadvm ui`, and shell completions.
- **Cross-platform** — CI on Linux/macOS/Windows and prebuilt release binaries.
- **Docs** — user guide and API reference published online.

## Next

- Exact topological face correspondence (not just volumetric / heuristic).
- A staging index and richer merge tooling.
