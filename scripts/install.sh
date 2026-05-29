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

usage() {
  cat <<EOF
Usage: $0 [--no-sign] [--no-load]

Build, optionally sign, install, depmod, and enable $module_name for boot.

Options:
  --no-sign   Skip scripts/sign_for_secure_boot.sh
  --no-load   Install and enable for boot, but do not load immediately
EOF
}

while [ "$#" -gt 0 ]; do
  case "$1" in
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

if ! command -v sudo >/dev/null 2>&1; then
  echo "Missing sudo; install must be able to write to /lib/modules and /etc." >&2
  exit 1
fi

sudo -v

if [ "$sign_module" -eq 1 ]; then
  "$repo_dir/scripts/build.sh" --sign
else
  "$repo_dir/scripts/build.sh"
fi

sudo install -D -m 0644 "$module" "$install_path"
echo "$module_name" | sudo tee "$modules_load_conf" >/dev/null
sudo depmod "$kernel_release"

if [ "$load_after_install" -eq 1 ]; then
  if lsmod | awk '{print $1}' | grep -qx "$module_name"; then
    sudo modprobe -r "$module_name"
  fi

  sudo modprobe "$module_name"
fi

cat <<EOF
Installed: $install_path
Autoload:  $modules_load_conf

Verify:
  cat /sys/bus/wmi/devices/99D89064-8D50-42BB-BEA9-155B2E5D0FCD/current_mode
EOF
