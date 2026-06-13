# Live demo

Below is a **real `cadvm view` output**. Between the two versions of this block a
hole was **moved** from the left to the right, and a **boss** was added on top.
It is rendered live in your browser; nothing is installed or sent anywhere, the
whole 3D scene is embedded in the page.

The unchanged body is drawn **translucent grey** so you can see the changes —
including the ones *inside* the part:

- **green** — added: the new hole (visible through the body) and the boss on top;
- **red** — removed: the old hole on the left;
- **grey** — unchanged faces (the block body keeps the same surfaces).

Drag to rotate, scroll to zoom, and toggle any layer in the panel.

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
