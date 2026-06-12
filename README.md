# cadvm — CAD Version Manager

`cadvm` is a **local-first version manager for STEP/STP CAD files**. It brings
Git-like workflows — snapshots, branches, diff, checkout, revert — to CAD data,
with a content-addressed, deduplicated object store.

It goes further than text versioning: a **geometric diff** (added / removed /
common material) via Open CASCADE, a **self-contained 3D viewer**, and an
**interactive terminal dashboard**.

## Quick install

```bash
# 1. The cadvm binary (VCS + TUI) — pure Rust, all platforms:
cargo install --path crates/cadvm-cli

# 2. (optional) Geometry features (`geom-diff`, `view`) need Open CASCADE.
#    You install OCCT yourself; cadvm does not bundle it. On Ubuntu/Debian:
sudo apt-get install -y libocct-foundation-dev libocct-modeling-data-dev \
    libocct-modeling-algorithms-dev libocct-data-exchange-dev cmake g++
cpp/build.sh
export CADVM_GEOM_BIN="$PWD/cpp/cadvm-geom/build/cadvm-geom"
```

The VCS and TUI work with step 1 alone. **Open CASCADE is a user-provided
prerequisite** for the geometry features — see [Installation](docs/src/installation.md)
for macOS/Windows. Full docs in [`docs/`](docs/).

## Project status

- ✅ 100% Rust, no FFI, no external services.
- ✅ Full snapshot / status / log / diff / branch / switch / checkout / revert / gc.
- ✅ Content-addressed storage (BLAKE3) with two-level deduplication.
- ✅ Lightweight STEP metadata extraction (schema, entity counts, top types).
- 🧩 Geometric diff via a C++/OCCT subprocess helper (`cadvm geom-diff`).
- 🎨 Self-contained 3D WebGL viewer of the diff (`cadvm view`).

## Architecture

A Cargo workspace with three crates:

| Crate         | Responsibility                                                        |
|---------------|-----------------------------------------------------------------------|
| `cadvm-store` | Content-addressed storage: `ObjectId`, BLAKE3 hashing, blobs, chunks. |
| `cadvm-core`  | Repository logic: commits, manifests, refs, branches, status, diff.   |
| `cadvm-cli`   | The `cadvm` binary: argument parsing (clap) and terminal output.      |

## Interactive dashboard

`cadvm ui` opens a full-screen terminal dashboard (built with `ratatui`):

- a **source-control-style commit list** (graph marker, short hash, branch chips,
  `HEAD` badge, author, relative time) with a live-detail pane (files + STEP
  metadata) on the right;
- press **`m`** to anchor a commit, then **`d`** (metadata diff), **`g`**
  (geometric diff), or **`v`** (build & open the 3D viewer) compare *anchor →
  selected* — or *parent → selected* if no anchor is set;
- **`b`** switch branch, **`s`** status, **`?`** help, **`q`** quit.

Geometry actions (`g`, `v`) need the `cadvm-geom` helper (see below).

## Installation

```bash
# Build everything
cargo build --release

# The binary is at target/release/cadvm
./target/release/cadvm --help

# Or install into ~/.cargo/bin
cargo install --path crates/cadvm-cli
```

Requires a recent stable Rust toolchain (tested on 1.96).

## Commands

| Command                       | Description                                               |
|-------------------------------|-----------------------------------------------------------|
| `cadvm init`                  | Create a `.cadvm/` repository in the current directory.   |
| `cadvm snapshot -m "msg"`     | Record a snapshot (commit) of all tracked STEP/STP files. |
| `cadvm ui`                    | Interactive full-screen terminal dashboard.               |
| `cadvm status`                | Show new / modified / deleted files vs. HEAD.             |
| `cadvm log`                   | Show the commit history of HEAD.                          |
| `cadvm show [<rev>]`          | Show one commit's details and per-file metadata.          |
| `cadvm diff`                  | Diff `HEAD~1..HEAD`.                                       |
| `cadvm diff <rev_a> <rev_b>`  | Diff two revisions.                                       |
| `cadvm checkout <rev>`        | Restore the working tree to a revision (restore-like).    |
| `cadvm checkout <rev> -- <file>…` | Restore only the named files (nothing is deleted).    |
| `cadvm branch`                | List branches.                                            |
| `cadvm branch <name>`         | Create a branch at HEAD.                                  |
| `cadvm branch -d <name>`      | Delete a branch (not the current one).                    |
| `cadvm switch <name>`         | Switch branches, restoring their files.                   |
| `cadvm revert <rev>`          | Create a commit that restores HEAD's parent state.        |
| `cadvm gc [--dry-run\|--prune]` | Report (and optionally delete) unreferenced objects.    |
| `cadvm config [<key>] [<value>]` | Get / set / list config (e.g. `user.name`).            |
| `cadvm geom-diff <rev_a> <rev_b>` | Geometric diff of modified STEP files (needs `cadvm-geom`). |
| `cadvm view <rev_a> <rev_b>`   | Generate a standalone 3D HTML viewer of the diff (needs `cadvm-geom`). |

Tracked formats in V1: **`.step`** and **`.stp`** only. Hidden directories and
the `.cadvm/` directory are skipped during scanning, as are paths matching
`.cadvmignore` (see below).

### `.cadvmignore`

An optional `.cadvmignore` at the repository root excludes files from tracking,
one pattern per line:

```text
# comments and blank lines are ignored
*.bak            # glob on the file name (* and ? supported)
build/           # a directory and everything beneath it
/secret/old.step # leading "/" anchors to the repo root; "/" => full-path match
```

### Revisions

The revision resolver accepts:

- `HEAD`, `HEAD~1`, `HEAD~2`, … (and `HEAD^`)
- a branch name
- a full 64-char hash (with or without the `blake3:` prefix)
- an unambiguous short-hash prefix (ambiguity is reported as an error)

### Author & config

Commits record an author. Configure it once per repository:

```bash
cadvm config user.name  "Your Name"
cadvm config user.email "you@example.com"
cadvm config            # list all settings
```

Resolution order when stamping a commit is **environment → config → fallback**:

- `CADVM_AUTHOR_NAME` / `CADVM_AUTHOR_EMAIL` override the config (handy in CI);
- otherwise `user.name` / `user.email` from `.cadvm/config.json` are used;
- if nothing is set, the author falls back to `unknown` (snapshots never block).

The author is shown by `cadvm log` and `cadvm show`. Legacy commits written
before authors existed read back fine (their author is simply absent).

### Geometric diff (Step 2, C++/OCCT)

The metadata diff above never inspects geometry. Real CAD diffing lives in
`cpp/cadvm-geom`, a **standalone C++/Open CASCADE executable** the Rust core runs
as a subprocess (no FFI — OCCT stays fully isolated). It loads two STEP files,
computes the boolean decomposition of their solids, and reports volumes:

- `common`  — material in both (A ∩ B);
- `added`   — material in B not in A (B − A);
- `removed` — material in A not in B (A − B).

It also reports a **topological face-to-face diff**: faces of A and B are matched
by a coarse geometric signature (surface type + rounded area + centre of mass),
yielding counts of *common / added / removed* faces alongside the volumes.

**Build it** (Ubuntu/Debian):

```bash
sudo apt-get install -y libocct-foundation-dev libocct-modeling-data-dev \
    libocct-modeling-algorithms-dev libocct-data-exchange-dev cmake g++

cpp/build.sh
export CADVM_GEOM_BIN="$PWD/cpp/cadvm-geom/build/cadvm-geom"
```

**Use it:**

```bash
cadvm geom-diff HEAD~1 HEAD          # all modified STEP files
cadvm geom-diff HEAD~1 HEAD -- piece.step
```

For each modified file cadvm extracts both versions from the store to temp files,
runs the helper, and prints the volume deltas. The Rust workspace builds and
tests **without** OCCT; only `geom-diff`/`view` need the helper at runtime (they
print a clear hint if `cadvm-geom` is not found).

### 3D viewer (Step 3)

`cadvm view` turns the geometric diff into a **single self-contained HTML file**
with a hand-written WebGL renderer (no CDN, no server, fully offline). Layers can
be toggled; drag to rotate, scroll to zoom:

- the **full parts A and B** as a *translucent context* (B shown by default);
- the boolean pieces opaque on top — **common = grey, added = green,
  removed = red**.

This shows *where* material was added/removed within the whole part, not just
isolated fragments.

```bash
cadvm view HEAD~1 HEAD                 # if exactly one STEP file changed
cadvm view HEAD~1 HEAD -- piece.step   # pick the file when several changed
cadvm view HEAD~1 HEAD --open          # also open it in the browser
cadvm view HEAD~1 HEAD -o diff.html    # choose the output path
```

Under the hood the `cadvm-geom mesh` subcommand tessellates the boolean pieces
(`BRepMesh_IncrementalMesh`) into triangle meshes, which the viewer embeds and
draws.

### Working-tree safety

`checkout`, `switch` and `revert` refuse to clobber your work:

- a **dirty working tree** blocks `switch` and `revert` (use `--force`);
- `checkout` refuses to overwrite a **locally modified file** (use `--force`);
- **untracked files are never deleted**.

`checkout <rev>` is intentionally **restore-like**: it restores files but does
**not** move the current branch or HEAD. Use `switch` to move between branches.

## Complete example

```bash
cadvm init

# piece.step = cube with a Ø5 hole
cadvm snapshot -m "Cube avec trou Ø5"

# edit piece.step = cube with a Ø8 hole
cadvm snapshot -m "Passage trou Ø5 vers Ø8"

cadvm log
cadvm diff HEAD~1 HEAD

# Undo the last change (creates a Revert commit)
cadvm revert HEAD
cadvm log

# Branch off and continue independently
cadvm branch second-hole
cadvm switch second-hole

# edit piece.step = two Ø5 holes
cadvm snapshot -m "Ajout deuxième trou Ø5"

cadvm switch main
cadvm switch second-hole
```

## Storage layout

A repository lives in `.cadvm/`:

```text
.cadvm/
├── objects/
│   ├── blobs/        # legacy V1 whole-file blobs (reclaimed by gc)
│   ├── chunks/       # fixed 256 KiB chunks — the V2 content store
│   ├── manifests/    # serialized snapshots
│   └── commits/      # serialized commits
├── refs/heads/<branch>   # each file holds the branch's tip commit id
├── HEAD                  # "ref: refs/heads/main" or a detached commit id
├── index.json            # reserved for a future staging area
└── tmp/                  # scratch space for atomic writes
```

Objects are addressed by the BLAKE3 hash of their content and sharded by the
first two hex byte-pairs:

```text
.cadvm/objects/blobs/ab/cd/<full-hex>
```

### Deduplication

1. **Content identity (level 1):** `raw_hash` is the BLAKE3 hash of the whole
   file. Identical files share the same identity, so status/diff comparisons and
   manifest dedup are exact.
2. **Fixed-size chunking (level 2):** files are split into 256 KiB chunks, each
   stored content-addressed, so identical chunks are shared across files and
   versions.

> **Storage note (V2, chunk-only).** File content is stored **only as chunks**;
> the whole file is *not* written as a standalone blob, so there is no on-disk
> duplication. `checkout` reconstructs each file by concatenating its chunks.
> This is backward compatible with the original V1 layout (which also wrote a
> redundant raw blob): V1 always stored the chunks too, so old repositories read
> back correctly, and `cadvm gc --prune` reclaims their now-unused raw blobs.

## STEP metadata (textual only)

cadvm does **not** parse geometry. It performs cheap text scanning to surface:

- line count;
- approximate `HEADER;` / `DATA;` section detection and line counts;
- `FILE_SCHEMA` extraction;
- entity count in the DATA section (`#N = TYPE(...)` definitions);
- the top 20 entity types by frequency.

## Development

```bash
cargo fmt
cargo test
cargo clippy --all-targets --all-features
```

Test fixtures live in [`tests/fixtures/`](tests/fixtures/).

### Documentation

- **User guide** (mdBook) — in [`docs/`](docs/):
  ```bash
  cargo install mdbook        # once
  mdbook serve docs --open    # live preview
  mdbook build docs           # static site → docs/book/
  ```
- **API / developer docs** (rustdoc):
  ```bash
  cargo doc --no-deps --workspace --open
  ```

## Limits

- Le diff géométrique (Step 2) calcule des **volumes** added/removed/common ; il
  ne fait pas encore de diff topologique face-à-face ni d'affichage rouge/vert/gris.
- cadvm ne sait pas encore merger deux modifications du même fichier STEP.
- `geom-diff` requiert le binaire `cadvm-geom` (OCCT) ; sans lui, le reste de
  cadvm fonctionne normalement.

## Roadmap

- **Step 2 (done):** the `cadvm-geom` C++/Open CASCADE helper —
  *added / removed / common* volumes + metrics, invoked by the Rust core.
- **Step 3 (done):** `cadvm view` — a self-contained WebGL HTML viewer rendering
  the diff in green/red/grey, with the full A/B parts as translucent context, plus
  a heuristic topological face-to-face diff.
- Later: a staging index, exact topological face correspondence, and richer merge
  tooling.
