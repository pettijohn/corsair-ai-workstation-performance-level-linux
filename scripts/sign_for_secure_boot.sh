#!/usr/bin/env bash
set -euo pipefail

src_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
key_dir="$src_dir/mok"
module="$src_dir/corsair_wmi.ko"
sign_file="/usr/src/linux-headers-$(uname -r)/scripts/sign-file"
key_base="$key_dir/corsair_wmi"

if [ ! -f "$key_base.key" ] && [ -f "$key_dir/corsair_wmi_probe.key" ]; then
  key_base="$key_dir/corsair_wmi_probe"
fi

if [ ! -f "$module" ]; then
  echo "Missing module: $module" >&2
  echo "Run: $src_dir/scripts/build.sh" >&2
  exit 1
fi

if [ ! -x "$sign_file" ]; then
  echo "Missing kernel sign-file helper: $sign_file" >&2
  exit 1
fi

mkdir -p "$key_dir"

if [ ! -f "$key_base.key" ] || [ ! -f "$key_base.der" ]; then
  key_base="$key_dir/corsair_wmi"
  openssl req \
    -new \
    -x509 \
    -newkey rsa:2048 \
    -keyout "$key_base.key" \
    -outform DER \
    -out "$key_base.der" \
    -nodes \
    -days 36500 \
    -subj "/CN=Local CORSAIR WMI Module Signing/"
fi

"$sign_file" sha256 \
  "$key_base.key" \
  "$key_base.der" \
  "$module"

echo "Signed: $module"
echo "Certificate: $key_base.der"
echo
echo "If Secure Boot is enabled, enroll the certificate once with:"
echo "  sudo mokutil --import '$key_base.der'"
echo
echo "Then reboot, choose Enroll MOK in the blue firmware screen, and load:"
echo "  sudo insmod '$module'"
