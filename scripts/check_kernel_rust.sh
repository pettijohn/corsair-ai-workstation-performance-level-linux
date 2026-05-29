#!/usr/bin/env bash
set -euo pipefail

repo_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
kdir="${KDIR:-/lib/modules/$(uname -r)/build}"

# Kernel Rust artifacts are compiler-version sensitive. Prefer the distro
# compiler that matches linux-lib-rust over a rustup toolchain in the shell.
if [ -x /usr/bin/rustc ]; then
  export PATH="/usr/bin:/bin:/usr/sbin:/sbin:$PATH"
fi

rust_src="${RUST_LIB_SRC:-$(rustc --print sysroot)/lib/rustlib/src/rust/library}"
if [ ! -d "$rust_src/core/src" ] && [ -d /opt/rustc-1.93.1/library/core/src ]; then
  rust_src="/opt/rustc-1.93.1/library"
fi
if [ ! -d "$rust_src/core/src" ] && [ -d /home/vscode/.rustup/toolchains/1.93.1-x86_64-unknown-linux-gnu/lib/rustlib/src/rust/library/core/src ]; then
  rust_src="/home/vscode/.rustup/toolchains/1.93.1-x86_64-unknown-linux-gnu/lib/rustlib/src/rust/library"
fi
rust_lib_dir="$kdir/rust"

echo "Kernel build dir: $kdir"
echo "Rust compiler:    $(command -v rustc) ($(rustc --version))"
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

echo "Building corsair_wmi driver module..."
RUST_LIB_SRC="$rust_src" make -C "$repo_dir/rust/corsair_wmi_kernel"
cp "$repo_dir/rust/corsair_wmi_kernel/corsair_wmi.ko" "$repo_dir/corsair_wmi.ko"
echo "Built: $repo_dir/corsair_wmi.ko"
