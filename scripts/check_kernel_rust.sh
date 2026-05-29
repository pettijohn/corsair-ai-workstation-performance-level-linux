#!/usr/bin/env bash
set -euo pipefail

repo_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
kdir="${KDIR:-/lib/modules/$(uname -r)/build}"
rust_src="${RUST_LIB_SRC:-$(rustc --print sysroot)/lib/rustlib/src/rust/library}"
rust_lib_dir="$kdir/rust"

echo "Kernel build dir: $kdir"
echo "Rust source dir:  $rust_src"

if [ ! -d "$rust_src/core/src" ]; then
  echo "Missing Rust standard-library source at: $rust_src" >&2
  exit 1
fi

RUST_LIB_SRC="$rust_src" make -C "$kdir" rustavailable

missing=0
for lib in libcore.rmeta libkernel.rmeta libpin_init.rmeta; do
  if [ ! -e "$rust_lib_dir/$lib" ]; then
    echo "Missing: $rust_lib_dir/$lib" >&2
    missing=1
  fi
done

if [ "$missing" -ne 0 ]; then
  cat >&2 <<EOF

Rust is enabled for this kernel, but the prebuilt Rust kernel libraries are not
visible in the header tree. On Ubuntu, install the matching host package, e.g.:

  sudo apt install linux-lib-rust-$(uname -r)

Then rebuild/reopen the dev container so /usr/src exposes that package.
EOF
  exit 1
fi

echo "Building Rust kernel smoke module..."
RUST_LIB_SRC="$rust_src" make -C "$repo_dir/rust_kernel_probe"
echo "Built: $repo_dir/rust_kernel_probe/corsair_wmi_rust_probe.ko"
