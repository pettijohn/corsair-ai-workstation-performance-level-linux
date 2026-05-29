#!/usr/bin/env bash
set -euo pipefail

repo_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
kernel_release="${KERNELRELEASE:-$(uname -r)}"
module_name="corsair_wmi"
module="$repo_dir/$module_name.ko"
install_dir="/lib/modules/$kernel_release/extra"
install_path="$install_dir/$module_name.ko"
modules_load_conf="/etc/modules-load.d/$module_name.conf"

load_after_install=1
sign_module=1
build_module=1

usage() {
  cat <<EOF
Usage: $0 [--no-build] [--no-sign] [--no-load]

Build, optionally sign, install, depmod, and enable $module_name for boot.

Options:
  --no-build  Install the existing $module_name.ko without building or signing
  --no-sign   Build without signing before install
  --no-load   Install and enable for boot, but do not load immediately

Note: --no-load still writes to /lib/modules and /etc/modules-load.d, so it
still needs root privileges for the install step.
EOF
}

while [ "$#" -gt 0 ]; do
  case "$1" in
    --no-build)
      build_module=0
      sign_module=0
      ;;
    --no-sign)
      sign_module=0
      ;;
    --no-load)
      load_after_install=0
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

sudo_cmd=()
if [ "${EUID:-$(id -u)}" -ne 0 ]; then
  if ! command -v sudo >/dev/null 2>&1; then
    echo "Missing sudo; install must be able to write to /lib/modules and /etc." >&2
    exit 1
  fi
  sudo_cmd=(sudo)
fi

if [ "$build_module" -eq 1 ]; then
  if [ "$sign_module" -eq 1 ]; then
    "$repo_dir/scripts/build.sh" --sign
  else
    "$repo_dir/scripts/build.sh"
  fi
fi

if [ ! -f "$module" ]; then
  echo "Missing module: $module" >&2
  if [ "$build_module" -eq 0 ]; then
    echo "Build and sign it first, or rerun without --no-build." >&2
  else
    echo "Build did not produce the expected module." >&2
  fi
  exit 1
fi

if [ "${#sudo_cmd[@]}" -ne 0 ]; then
  "${sudo_cmd[@]}" -v
fi

"${sudo_cmd[@]}" install -D -m 0644 "$module" "$install_path"
echo "$module_name" | "${sudo_cmd[@]}" tee "$modules_load_conf" >/dev/null
"${sudo_cmd[@]}" depmod "$kernel_release"

if [ "$load_after_install" -eq 1 ]; then
  if lsmod | awk '{print $1}' | grep -qx "$module_name"; then
    "${sudo_cmd[@]}" modprobe -r "$module_name"
  fi

  "${sudo_cmd[@]}" modprobe "$module_name"
fi

cat <<EOF
Installed: $install_path
Autoload:  $modules_load_conf

Verify:
  cat /sys/bus/wmi/devices/99D89064-8D50-42BB-BEA9-155B2E5D0FCD/current_mode
EOF
