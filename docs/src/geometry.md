# Geometric diff

The metadata diff (`cadvm diff`) never inspects geometry. Real CAD diffing lives
in **`cpp/cadvm-geom`**, a standalone C++/Open CASCADE executable that the Rust
core runs as a subprocess (no FFI — OCCT stays isolated in this one binary).

See [Installation](installation.md#2-the-geometry-helper-optional) to build it.

## What it computes

Given two STEP files A and B, it computes the boolean decomposition of their
solids:

- **common** — material in both (A ∩ B);
- **added** — material in B not in A (B − A);
- **removed** — material in A not in B (A − B).

For each it reports a **volume** and face count, plus per-input metrics (volume,
surface area, solid/shell/face counts, bounding box).

It also reports a **topological face-to-face diff**: faces of A and B are matched
by their **underlying surface** — the plane equation, the cylinder's axis and
radius, the cone/sphere/torus parameters — which is invariant to how the face is
trimmed. A wall that merely gains a hole keeps the same plane and counts as
*unchanged*; only genuinely new or removed surfaces (e.g. the hole's cylinder)
are reported as *added* / *removed*. (Freeform B-spline faces fall back to an
area + centroid signature.)

## Using it

```bash
cadvm geom-diff HEAD~1 HEAD
```

```text
Geometric diff 50d54d61..62cda376

  piece.step
    volume:  6498.700 -> 6584.340
    area:    2709.870 -> 2902.720
    bodies:  76 shells -> 190 shells
    faces:   76 -> 190
    bbox:    20.00×20.00×20.00 -> 20.00×20.00×20.00
    common:  vol 6266.980 (157 faces)
    added:   vol 173.788 (252 faces)
    removed: vol 109.907 (179 faces)
    faces (topo): 2 common, 188 added, 74 removed
```

By default it diffs every modified STEP file; restrict it with `-- <file>`.

> **Solids vs shells.** Many STEP exports are *sewn shells* rather than OCCT
> `solid`s, so `bodies` may report shells. Volumes are still integrated
> correctly over the faces.

## The JSON contract

`cadvm-geom diff a.step b.step` prints a JSON object to stdout (`status`,
`file_a/b`, `a`/`b` metrics, `common`/`added`/`removed` pieces, `faces_topo`). A
handled geometry failure prints `{"status":"error", ...}` and still exits 0, so
the caller always receives structured output.

To visualize the diff in 3D, see [3D viewer](viewer.md).
