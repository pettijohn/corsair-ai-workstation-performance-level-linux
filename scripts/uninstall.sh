#!/usr/bin/env bash
set -euo pipefail

kernel_release="${KERNELRELEASE:-$(uname -r)}"
module_name="corsair_wmi"
install_path="/lib/modules/$kernel_release/extra/$module_name.ko"
modules_load_conf="/etc/modules-load.d/$module_name.conf"

if ! command -v sudo >/dev/null 2>&1; then
  echo "Missing sudo; uninstall must be able to write to /lib/modules and /etc." >&2
  exit 1
fi

sudo -v

if lsmod | awk '{print $1}' | grep -qx "$module_name"; then
  sudo modprobe -r "$module_name"
fi

sudo rm -f "$install_path" "$modules_load_conf"
sudo depmod "$kernel_release"

echo "Removed: $install_path"
echo "Removed: $modules_load_conf"
