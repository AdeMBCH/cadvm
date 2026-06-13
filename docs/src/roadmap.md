# Limits & roadmap

## Current limits

- The geometric diff matches faces by their underlying analytic surface (plane,
  cylinder, cone, sphere, torus); freeform B-spline faces use a coarser
  area + centroid fallback.
- cadvm cannot merge two concurrent edits of the same STEP file.
- `geom-diff` / `view` require the `cadvm-geom` helper (OCCT). The rest of cadvm
  works without it.

## Done

- **VCS core** — snapshots, log, status, diff, branches, switch, revert,
  checkout, gc, config/author, deduplicated, gzip-compressed chunk storage.
- **Geometric diff** — `cadvm-geom` (C++/OCCT): boolean volumes + metrics +
  surface-based face-to-face diff.
- **3D viewer** — `cadvm view`: self-contained WebGL HTML, per-face
  green/red/grey changes.
- **Interactive TUI** — `cadvm ui`, and shell completions.
- **Cross-platform** — CI on Linux/macOS/Windows and prebuilt release binaries.
- **Docs** — user guide and API reference published online.

## Next

- Mesh-based geometric diff and 3D viewer for STL/OBJ (today they are versioned
  with metadata only; the B-Rep volume/face diff applies to STEP/STP).

- Exact topological face correspondence (not just volumetric / heuristic).
- A staging index and richer merge tooling.
