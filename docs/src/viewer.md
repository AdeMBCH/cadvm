# 3D viewer

```bash
cadvm view HEAD~1 HEAD --open
```

`cadvm view` turns a geometric diff into a **single self-contained HTML file**
with a hand-written WebGL renderer — **no CDN, no server, fully offline**. Open
it in any modern browser.

## What you see

- The **full parts A and B** as a *translucent context* (B shown by default).
- The boolean pieces opaque on top:
  - **common = grey**, **added = green**, **removed = red**.

This shows *where* material was added or removed within the whole part, not just
isolated fragments. Each layer can be toggled in the side panel.

**Controls:** drag to rotate · scroll to zoom · toggle layers with the checkboxes.

## Options

```bash
cadvm view HEAD~1 HEAD                 # if exactly one STEP file changed
cadvm view HEAD~1 HEAD -- piece.step   # pick the file when several changed
cadvm view HEAD~1 HEAD -o diff.html    # choose the output path
cadvm view HEAD~1 HEAD --open          # also open in the default browser
```

You can also launch it straight from the [TUI](tui.md) with the `v` key.

## Under the hood

`cadvm-geom mesh a.step b.step out.json` tessellates the boolean pieces and the
full shapes (`BRepMesh_IncrementalMesh`) into flat-shaded triangle meshes. cadvm
embeds that JSON into the HTML template (`cadvm-cli/src/viewer.rs`) and the WebGL
code renders it with per-layer colors and transparency.
