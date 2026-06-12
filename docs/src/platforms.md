# Platform support

| Component | Linux | macOS | Windows |
|-----------|:-----:|:-----:|:-------:|
| **VCS + TUI** (`init`, `snapshot`, `ui`, …) | ✅ | ✅ | ✅ |
| **Geometry** (`geom-diff`, `view`) | ✅ | ✅ | ✅ |

## VCS core & TUI

Pure Rust and cross-platform by design. Platform-specific bits (the `SIGPIPE`
reset, opening the browser) are gated per-OS, so the binary builds and runs on
Linux, macOS and Windows. The TUI works in any modern terminal (Windows
Terminal, Terminal.app, common Linux terminals).

## Geometry helper

`cadvm-geom` works wherever **Open CASCADE** and a C++17 toolchain are available
— all three OSes qualify — but the *installation* of OCCT differs per platform
(see [Installation](installation.md)). The VCS and TUI work without it; only
`geom-diff` and `view` require it at runtime.

## Tested status

cadvm has so far been **built and tested on Ubuntu 24.04 (OCCT 7.6)**. The code
is written to be portable and *should* build on macOS and Windows, but those
targets are **not yet verified** in CI. If you try them, expect to adjust the
OCCT toolkit names or CMake discovery in `cpp/cadvm-geom/CMakeLists.txt`.

> Multi-OS continuous integration (to produce verified, prebuilt binaries) is on
> the [roadmap](roadmap.md).
