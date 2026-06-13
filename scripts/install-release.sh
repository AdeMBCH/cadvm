#!/bin/sh
# One-line installer for the prebuilt cadvm binary (no Rust needed).
#
#   curl -fsSL https://raw.githubusercontent.com/AdeMBCH/cadvm/main/scripts/install-release.sh | sh
#
# Detects your OS/arch, downloads the matching binary from the latest GitHub
# release, and installs it (default: ~/.local/bin). Override with:
#   CADVM_INSTALL_DIR=/usr/local/bin
#
# This installs the version control + TUI. The geometry features (geom-diff,
# view) still need Open CASCADE — see https://adembch.github.io/cadvm/
set -eu

repo="AdeMBCH/cadvm"
install_dir="${CADVM_INSTALL_DIR:-$HOME/.local/bin}"

os="$(uname -s)"
arch="$(uname -m)"

case "$os/$arch" in
  Linux/x86_64)            target="x86_64-unknown-linux-gnu" ;;
  Darwin/arm64|Darwin/aarch64) target="aarch64-apple-darwin" ;;
  *)
    echo "cadvm: no prebuilt binary for $os/$arch." >&2
    echo "Build from source instead: https://adembch.github.io/cadvm/installation.html" >&2
    exit 1
    ;;
esac

url="https://github.com/${repo}/releases/latest/download/cadvm-${target}"
echo "Downloading cadvm ($target)…"
mkdir -p "$install_dir"
tmp="$(mktemp)"
if command -v curl >/dev/null 2>&1; then
  curl -fSL "$url" -o "$tmp"
elif command -v wget >/dev/null 2>&1; then
  wget -qO "$tmp" "$url"
else
  echo "cadvm: need curl or wget to download." >&2
  exit 1
fi

chmod +x "$tmp"
mv "$tmp" "$install_dir/cadvm"
echo "Installed: $install_dir/cadvm"

# Friendly PATH hint.
case ":$PATH:" in
  *":$install_dir:"*) : ;;
  *)
    echo
    echo "Note: $install_dir is not on your PATH. Add it, e.g.:"
    echo "    export PATH=\"$install_dir:\$PATH\""
    ;;
esac

echo
echo "Run:  cadvm --help"
