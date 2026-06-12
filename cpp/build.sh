#!/usr/bin/env bash
# Build the cadvm-geom C++/OCCT helper.
#
# Prerequisite (Ubuntu/Debian):
#   sudo apt-get install -y libocct-foundation-dev libocct-modeling-data-dev \
#       libocct-modeling-algorithms-dev libocct-data-exchange-dev cmake g++
#
# Usage:
#   cpp/build.sh
#
# The binary is produced at cpp/cadvm-geom/build/cadvm-geom. Point the Rust CLI
# at it with:
#   export CADVM_GEOM_BIN="$PWD/cpp/cadvm-geom/build/cadvm-geom"
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
src="$here/cadvm-geom"
build="$src/build"

cmake -S "$src" -B "$build" -DCMAKE_BUILD_TYPE=Release
cmake --build "$build" --parallel

echo
echo "Built: $build/cadvm-geom"
echo "Use it with: export CADVM_GEOM_BIN=\"$build/cadvm-geom\""
