#!/usr/bin/env bash
# Uninstall cadvm in one command.
#
#   ./scripts/uninstall.sh
#
# Removes the `cadvm` binary and the locally built geometry helper. Your
# repositories' `.cadvm/` directories are left completely untouched.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

echo "==> Removing the cadvm binary…"
if cargo uninstall cadvm-cli >/dev/null 2>&1; then
  echo "    Removed ~/.cargo/bin/cadvm"
else
  echo "    cadvm-cli was not installed via cargo (nothing to remove there)."
fi

if [ -d "$root/cpp/cadvm-geom/build" ]; then
  rm -rf "$root/cpp/cadvm-geom/build"
  echo "==> Removed the cadvm-geom build directory."
fi

echo
echo "cadvm has been uninstalled."
echo "  • Your repositories' .cadvm/ data is untouched."
echo "  • If you added 'export CADVM_GEOM_BIN=...' to your shell profile, remove it."
