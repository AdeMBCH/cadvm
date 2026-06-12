# cadvm agent instructions

## Scope
cadvm is a local-first version manager for STEP/STP CAD files.

- **Rust core (Step 1, done):** the VCS — commits, manifests, refs, branches,
  status, diff, checkout, switch, revert, gc, config/author. Pure Rust.
- **Geometry helper (Step 2, done):** `cpp/cadvm-geom`, a standalone
  C++/Open CASCADE executable. `diff` computes added/removed/common volumes +
  metrics; `mesh` tessellates the boolean pieces to triangle JSON. The Rust core
  invokes it **as a subprocess** (no FFI). OCCT stays isolated in this one binary.
- **3D viewer (Step 3, done):** `cadvm view` emits a single self-contained
  HTML file with a hand-written WebGL renderer (no CDN, no server, no framework).
  The template lives in `cadvm-cli/src/viewer.rs`.
- **Interactive TUI (done):** `cadvm ui`, a ratatui dashboard in
  `cadvm-cli/src/ui.rs`. It calls `cadvm-core` directly (it is the same binary);
  geometry actions shell out to `cadvm-geom` via the core's `geom` module.

## Architecture
- `cadvm-cli`: CLI only.
- `cadvm-core`: repository logic; `geom` module shells out to the helper.
- `cadvm-store`: content-addressed storage (chunk-only V2).
- `cpp/cadvm-geom`: C++/OCCT geometry helper (subprocess, JSON over stdout).

## Do not add
- FFI bindings to OCCT (keep it a subprocess).
- C++ anywhere except `cpp/cadvm-geom`.
- A frontend framework, bundler, CDN dependency or web server for the viewer —
  it must stay a single self-contained, offline HTML file.
- cloud sync, external database, Git backend.
- STL / OBJ / native-CAD format support.

## Safety
Never overwrite user files unless `--force` is explicitly passed.
Prefer conservative behavior over destructive behavior.

## Commands
Rust:
- cargo fmt
- cargo test
- cargo clippy --all-targets --all-features

C++ helper (needs OCCT, see below):
- cpp/build.sh
- Point the CLI at it: export CADVM_GEOM_BIN="$PWD/cpp/cadvm-geom/build/cadvm-geom"

## OCCT prerequisite (Ubuntu/Debian)
    sudo apt-get install -y libocct-foundation-dev libocct-modeling-data-dev \
        libocct-modeling-algorithms-dev libocct-data-exchange-dev cmake g++

The Rust workspace builds and tests **without** OCCT; only the geometry helper
and `cadvm geom-diff` require it at runtime.
