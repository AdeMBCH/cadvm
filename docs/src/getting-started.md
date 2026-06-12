# Getting started

This walks through a full session on a real part.

## Create a repository

```bash
mkdir my-part && cd my-part
cadvm init
```

This creates a `.cadvm/` directory. cadvm tracks **`.step` and `.stp` files**
recursively from here (other files are ignored — see
[Storage model](storage.md)).

## Set your identity (optional but recommended)

```bash
cadvm config user.name  "Your Name"
cadvm config user.email "you@example.com"
```

Commits made afterwards record this author. You can also override it per-command
with the `CADVM_AUTHOR_NAME` / `CADVM_AUTHOR_EMAIL` environment variables.

## First snapshot

Drop a STEP file in the directory, then:

```bash
cadvm status          # piece.step shows under "New"
cadvm snapshot -m "Initial version"
```

A *snapshot* captures the whole working tree (there is no staging step in V1).

## Make a change and snapshot again

Re-export the part from your CAD tool over the same file, then:

```bash
cadvm status          # piece.step shows under "Modified"
cadvm snapshot -m "Enlarged the main bore"
```

## Inspect history

```bash
cadvm log             # commits, newest first, with author + date
cadvm show HEAD       # one commit in detail + per-file STEP metadata
cadvm diff HEAD~1 HEAD # metadata diff (size, lines, entities, schema)
```

## See the geometry change

With the [geometry helper](geometry.md) built:

```bash
cadvm geom-diff HEAD~1 HEAD   # added / removed / common volumes + face counts
cadvm view HEAD~1 HEAD --open # 3D diff in your browser
```

## Or do it all interactively

```bash
cadvm ui
```

A full-screen dashboard to browse commits and launch diffs/viewer — see
[Interactive dashboard](tui.md).

Next: [Typical workflow](workflow.md).
