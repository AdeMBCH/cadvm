# Limits & roadmap

## Current limits

- The geometric diff matches faces by their underlying analytic surface (plane,
  cylinder, cone, sphere, torus); freeform B-spline faces use a coarser
  area + centroid fallback.
- cadvm cannot merge two concurrent edits of the same file.
- The STL/OBJ mesh diff depends on tessellation and a distance tolerance, so it
  is fuzzier than the B-Rep diff.
- `geom-diff` / `view` need the `cadvm-geom` helper (OCCT) **only for STEP/STP**;
  STL/OBJ diff in pure Rust. The rest of cadvm works without OCCT.

## Done

- **VCS core** — snapshots, log, status, diff, branches, switch, revert,
  checkout, gc, config/author, deduplicated, gzip-compressed chunk storage.
- **Geometric diff** — `cadvm-geom` (C++/OCCT): boolean volumes + metrics +
  surface-based face-to-face diff.
- **3D viewer** — `cadvm view`: self-contained WebGL HTML, per-face
  green/red/grey changes.
- **STL/OBJ support** — versioning + mesh metadata, and a pure-Rust
  distance-based mesh diff feeding the same 3D viewer (no Open CASCADE).
- **Interactive TUI** — `cadvm ui`, and shell completions.
- **Cross-platform** — CI on Linux/macOS/Windows and prebuilt release binaries.
- **Docs** — user guide and API reference published online.

## Next

- A `verify` command that asserts expected geometric deltas (for AI gating/evals).
- An MCP server exposing diff/version as a tool agents can call.
- Sharper mesh diff (point sampling beyond centroids, configurable tolerance).
- glTF/PLY mesh formats.
- A staging index and richer merge tooling.
