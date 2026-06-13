# Live demo

Below is a **real `cadvm view` output** — the geometric diff between two versions
of the same block: the first has one hole, the second has **a second hole added**.
It is rendered live in your browser; nothing is installed or sent anywhere, the
whole 3D scene is embedded in the page.

Most of the block is **grey** (unchanged); the faces touched by the new hole show
up in **green** (added) and **red** (removed).

- **Drag** to rotate · **scroll** to zoom
- Each **face** is colored by how it changed: **grey** = unchanged, **green** =
  added in the new version, **red** = removed (present only in the old one)
- Toggle any layer with the checkboxes (top-left)

<iframe
  src="demo-viewer.html"
  title="cadvm — interactive 3D geometric diff"
  width="100%"
  height="560"
  style="border:1px solid #3a3d44; border-radius:8px; background:#1e2024;"
  loading="lazy">
</iframe>

<p>
  <a href="demo-viewer.html" target="_blank">↗ Open the demo full-screen</a>
</p>

This page is exactly what `cadvm view HEAD~1 HEAD` produces — a single,
self-contained HTML file. See [3D viewer](viewer.md) for how to generate one for
your own parts, and [Geometric diff](geometry.md) for the numbers behind it.
