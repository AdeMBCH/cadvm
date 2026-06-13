# 3D viewer

```bash
cadvm view HEAD~1 HEAD --open
```

`cadvm view` turns a geometric diff into a **single self-contained HTML file**
with a hand-written WebGL renderer — **no CDN, no server, fully offline**. Open
it in any modern browser.

## What you see

Each **face** of the part is colored by how it changed:

- **grey** — unchanged (the face exists in both versions);
- **green** — added (a face of the new version with no match in the old one);
- **red** — removed (a face of the old version, gone in the new one).

So you see exactly *which faces* changed on the real part. Each layer can be
toggled in the side panel.

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

`cadvm-geom mesh a.step b.step out.json` classifies each face (unchanged / added
/ removed) and tessellates them (`BRepMesh_IncrementalMesh`) into flat-shaded,
per-color triangle meshes. cadvm
embeds that JSON into the HTML template (`cadvm-cli/src/viewer.rs`) and the WebGL
code renders it with per-layer colors and transparency.
