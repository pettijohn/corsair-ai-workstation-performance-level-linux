#!/usr/bin/env bash
set -euo pipefail

repo_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
kdir="${KDIR:-/lib/modules/$(uname -r)/build}"
module_dir="$repo_dir/rust/corsair_wmi_kernel"
sign_module=0

usage() {
  cat <<EOF
Usage: $0 [--sign]

Build corsair_wmi.ko.

Options:
  --sign   Sign the built module with scripts/sign_for_secure_boot.sh
EOF
}

while [ "$#" -gt 0 ]; do
  case "$1" in
    --sign)
      sign_module=1
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      usage >&2
      exit 2
      ;;
  esac
  shift
done

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

echo "Kernel build dir: $kdir"
echo "Rust compiler:    $(command -v rustc) ($(rustc --version))"
echo "Rust source dir:  $rust_src"

if [ ! -d "$rust_src/core/src" ]; then
  echo "Missing Rust standard-library source at: $rust_src" >&2
  exit 1
fi

RUST_LIB_SRC="$rust_src" make -C "$module_dir"
cp "$module_dir/corsair_wmi.ko" "$repo_dir/corsair_wmi.ko"
if [ "$sign_module" -eq 1 ]; then
  "$repo_dir/scripts/sign_for_secure_boot.sh" "$repo_dir/corsair_wmi.ko"
fi
echo "Built: $repo_dir/corsair_wmi.ko"
