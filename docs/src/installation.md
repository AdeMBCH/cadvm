# Installation

cadvm installs in two layers. The first is all most users need.

1. the **`cadvm` binary** (version control + TUI) — pure Rust, works everywhere;
2. the **`cadvm-geom` helper** (geometric diff + viewer) — requires **Open
   CASCADE**, a prerequisite you install yourself (cadvm does not bundle it).

You only need part 2 for `cadvm geom-diff` and `cadvm view`; everything else
works without Open CASCADE.

## 1. The `cadvm` binary

Requires a recent stable Rust toolchain (tested on 1.96).

```bash
# from a clone of the repository
cargo install --path crates/cadvm-cli
```

This puts `cadvm` in `~/.cargo/bin`. Make sure that directory is on your `PATH`:

```bash
export PATH="$HOME/.cargo/bin:$PATH"
cadvm --help
```

To update later, re-run the same command with `--force`.

## 2. Geometry features (Open CASCADE prerequisite)

### Install Open CASCADE

**Ubuntu / Debian:**

```bash
sudo apt-get install -y \
  libocct-foundation-dev libocct-modeling-data-dev \
  libocct-modeling-algorithms-dev libocct-data-exchange-dev \
  cmake g++
```

**macOS (Homebrew):**

```bash
brew install opencascade cmake
```

**Windows:** install OCCT (e.g. via vcpkg `opencascade`) and CMake, then build
from a Developer prompt.

### Build the helper

```bash
cpp/build.sh        # produces cpp/cadvm-geom/build/cadvm-geom
```

### Point cadvm at it

```bash
export CADVM_GEOM_BIN="$PWD/cpp/cadvm-geom/build/cadvm-geom"
```

Add that line to your `~/.bashrc` (or shell profile) to make it permanent. If
`cadvm-geom` is on your `PATH`, the env var is optional.

## Shell completions

`cadvm` can generate completion scripts for bash, zsh, fish, elvish and
PowerShell:

```bash
# bash
mkdir -p ~/.local/share/bash-completion/completions
cadvm completions bash > ~/.local/share/bash-completion/completions/cadvm

# zsh (ensure the dir is in your $fpath, then recompinit)
cadvm completions zsh > ~/.zfunc/_cadvm

# fish
cadvm completions fish > ~/.config/fish/completions/cadvm.fish
```

Reopen your shell, then `cadvm <Tab>` completes commands and options.

## Verifying

```bash
cadvm --help                 # lists all commands, including ui
cadvm completions bash | head
cadvm-geom diff a.step b.step  # if you built the helper
```

Next: [Getting started](getting-started.md).
