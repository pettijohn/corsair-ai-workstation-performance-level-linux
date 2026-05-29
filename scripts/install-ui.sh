#!/usr/bin/env bash
set -euo pipefail

repo_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
binary_name="corsair-mode-indicator"
binary="$repo_dir/target/release/$binary_name"
bindir="${HOME}/.local/bin"
icon_dir="${HOME}/.local/share/icons/hicolor/scalable/status"
applications_dir="${HOME}/.local/share/applications"
autostart_dir="${HOME}/.config/autostart"
launcher_file="${applications_dir}/corsair-mode-indicator.desktop"
autostart_file="${autostart_dir}/corsair-mode-indicator.desktop"

build_ui=1
install_ui=1

usage() {
  cat <<EOF
Usage: $0 [--build-only] [--no-build]

Build and/or install the Corsair Performance GNOME indicator.

Options:
  --build-only  Build the release binary and do not install user assets
  --no-build    Install the existing release binary without running cargo

Recommended split workflow:
  # In the dev container:
  $0 --build-only

  # On the host:
  $0 --no-build
EOF
}

desktop_exec_value() {
  case "$1" in
    *[[:space:]]*)
      local escaped
      escaped="${1//\\/\\\\}"
      escaped="${escaped//\"/\\\"}"
      printf '"%s"' "$escaped"
      ;;
    *)
      printf '%s' "$1"
      ;;
  esac
}

sed_replacement_value() {
  printf '%s' "$1" | sed 's/[&|\\]/\\&/g'
}

while [ "$#" -gt 0 ]; do
  case "$1" in
    --build-only)
      build_ui=1
      install_ui=0
      ;;
    --no-build)
      build_ui=0
      install_ui=1
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

if [ "$build_ui" -eq 1 ]; then
  if ! command -v cargo >/dev/null 2>&1; then
    echo "Missing cargo; build inside the dev container or rerun with --no-build." >&2
    exit 1
  fi

  cargo build --release --package "$binary_name" --manifest-path "$repo_dir/Cargo.toml"
fi

if [ "$install_ui" -eq 0 ]; then
  echo "Built: $binary"
  exit 0
fi

if [ ! -f "$binary" ]; then
  echo "Missing binary: $binary" >&2
  echo "Build it first in the dev container with: $0 --build-only" >&2
  exit 1
fi

autostart_enabled="true"
if [ -f "$autostart_file" ]; then
  existing_value="$(awk -F= 'tolower($1) == "x-gnome-autostart-enabled" {print tolower($2); found=1} END {if (!found) print ""}' "$autostart_file")"
  if [ "$existing_value" = "false" ]; then
    autostart_enabled="false"
  fi
fi

mkdir -p "$bindir" "$icon_dir" "$applications_dir" "$autostart_dir"
install -m 0755 "$binary" "$bindir/$binary_name"
install -m 0644 "$repo_dir"/icons/corsair-mode-*-symbolic.svg "$icon_dir/"
exec_value="$(desktop_exec_value "$bindir/$binary_name")"
exec_replacement="$(sed_replacement_value "$exec_value")"
sed "s|@EXEC@|$exec_replacement|g" "$repo_dir/packaging/corsair-mode-indicator.desktop.in" > "$launcher_file"
sed \
  -e "s|@EXEC@|$exec_replacement|g" \
  -e "s|@AUTOSTART_ENABLED@|$autostart_enabled|g" \
  "$repo_dir/packaging/corsair-mode-indicator-autostart.desktop.in" > "$autostart_file"
chmod 0644 "$launcher_file" "$autostart_file"

if command -v gtk-update-icon-cache >/dev/null 2>&1; then
  gtk-update-icon-cache -q "${HOME}/.local/share/icons/hicolor" || true
fi

cat <<EOF
Installed Corsair Performance indicator.
Binary:    $bindir/$binary_name
Launcher:  $launcher_file
Autostart: $autostart_file

Run it now with:
  $bindir/$binary_name
EOF
