# Developing

## Repository layout

```text
cadvm/
├── crates/
│   ├── cadvm-cli/    # the `cadvm` binary (CLI + TUI + viewer template)
│   ├── cadvm-core/   # repository logic, geom bridge
│   └── cadvm-store/  # content-addressed storage
├── cpp/cadvm-geom/   # C++/OCCT geometry helper
├── docs/             # this mdBook (user guide)
└── tests/fixtures/   # sample STEP files
```

## Everyday commands

```bash
cargo fmt
cargo test                       # geometry tests skip if CADVM_GEOM_BIN unset
cargo clippy --all-targets --all-features
```

To run the geometry-backed tests, point at a built helper:

```bash
export CADVM_GEOM_BIN="$PWD/cpp/cadvm-geom/build/cadvm-geom"
cargo test
```

## API documentation (`cargo doc`)

The crates are documented with `///` / `//!` doc comments. Generate the API
reference (for contributors) with:

```bash
cargo doc --no-deps --workspace --open
```

Add `--document-private-items` to include internal items. The build is warning
-clean under `RUSTDOCFLAGS="-D warnings"`.

This is the **developer/API** doc. The **user guide** is this mdBook (below) —
keep the two distinct: rustdoc explains the code, the book explains the tool.

## User guide (this book)

```bash
cargo install mdbook          # once
mdbook serve docs --open      # live-reloading preview
mdbook build docs             # static site in docs/book/
```

## The geometry helper

```bash
cpp/build.sh                  # needs Open CASCADE + cmake + a C++17 compiler
```

The Rust core talks to it only as a subprocess (see `cadvm-core::geom`); there is
no FFI. Keep OCCT confined to `cpp/cadvm-geom`.
