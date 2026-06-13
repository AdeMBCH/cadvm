#!/usr/bin/env bash
# Install cadvm from source in one command.
#
#   ./scripts/install.sh
#
# Installs the `cadvm` binary (version control + TUI) into ~/.cargo/bin. If Open
# CASCADE is detected, it also builds the optional geometry helper.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

echo "==> Installing the cadvm binary (cargo install)…"
cargo install --path crates/cadvm-cli --force

# Optionally build the geometry helper when Open CASCADE is available.
has_occt=false
if command -v cmake >/dev/null 2>&1; then
  if pkg-config --exists opencascade 2>/dev/null \
     || ls /usr/include/opencascade/STEPControl_Reader.hxx >/dev/null 2>&1 \
     || ls /usr/local/include/opencascade/STEPControl_Reader.hxx >/dev/null 2>&1 \
     || ls /opt/homebrew/include/opencascade/STEPControl_Reader.hxx >/dev/null 2>&1; then
    has_occt=true
  fi
fi

echo
if [ "$has_occt" = true ]; then
  echo "==> Open CASCADE detected — building the cadvm-geom helper…"
  ./cpp/build.sh
  echo
  echo "To enable geometry features, add this to your shell profile:"
  echo "    export CADVM_GEOM_BIN=\"$root/cpp/cadvm-geom/build/cadvm-geom\""
else
  echo "==> Open CASCADE not found — installed the version control + TUI only."
  echo "    For 'cadvm geom-diff' / 'cadvm view', install Open CASCADE then run"
  echo "    ./cpp/build.sh  (see https://adembch.github.io/cadvm/installation.html)."
fi

echo
echo "Done. Try:  cadvm --help"
