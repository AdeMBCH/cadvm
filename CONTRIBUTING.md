# Contributing to cadvm

Thanks for your interest in cadvm! Issues and pull requests are welcome.

## License

cadvm is released under the **Prosperity Public License 3.0.0** (see
[`LICENSE`](LICENSE)): free for noncommercial use, with a thirty-day commercial
trial. By submitting a contribution, you agree that it is provided under the
project's license.

You are free to reuse and modify cadvm as a base for noncommercial work. For
commercial use beyond the trial, contact the maintainer.

## Getting set up

```bash
cargo fmt
cargo test
cargo clippy --all-targets --all-features
```

The geometry tests skip automatically unless `CADVM_GEOM_BIN` points at a built
`cadvm-geom` helper. See the [Developing guide](https://adembch.github.io/cadvm/developing.html)
for the full setup (including the C++/Open CASCADE helper).

## Documentation

- **User guide:** <https://adembch.github.io/cadvm/>
- **API reference (rustdoc):** <https://adembch.github.io/cadvm/api/cadvm_core/index.html>
- **Live 3D demo:** <https://adembch.github.io/cadvm/demo.html>

Build the docs locally:

```bash
mdbook serve docs --open                 # user guide
cargo doc --no-deps --workspace --open   # API reference
```

## Before opening a PR

- `cargo fmt`, `cargo clippy` and `cargo test` all pass (CI enforces this).
- Keep the geometry/Open CASCADE code confined to `cpp/cadvm-geom` (the Rust
  core talks to it as a subprocess — no FFI).
- Update the docs in `docs/` when you change user-facing behavior.
