#!/usr/bin/env bash
set -euo pipefail

src_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
build_dir="/tmp/corsair_wmi_build"

rm -rf "$build_dir"
mkdir -p "$build_dir"
mkdir -p "$build_dir/src"
cp "$src_dir/Makefile" "$build_dir/"
cp "$src_dir/src/corsair_wmi.c" "$build_dir/src/"

make -C "$build_dir"
cp "$build_dir/corsair_wmi.ko" "$src_dir/"

echo "Built: $src_dir/corsair_wmi.ko"
