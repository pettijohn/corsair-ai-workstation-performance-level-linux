#!/usr/bin/env bash

set -euo pipefail

git config --global --add safe.directory "$(pwd)"

check_owner_path() {
  local path="$1"
  local expected_uid
  local expected_gid
  local actual_uid
  local actual_gid

  if [ ! -e "${path}" ]; then
    return
  fi

  expected_uid="$(id -u)"
  expected_gid="$(id -g)"
  actual_uid="$(stat -c '%u' "${path}")"
  actual_gid="$(stat -c '%g' "${path}")"

  if [ "${actual_uid}" != "${expected_uid}" ] || [ "${actual_gid}" != "${expected_gid}" ]; then
    echo "WARNING: ${path} is owned by ${actual_uid}:${actual_gid}, expected ${expected_uid}:${expected_gid}."
    echo "This can break VS Code server startup, extension installs, or shared editor state."
  fi
}

echo "Devcontainer setup complete."
echo "User: $(whoami)"
echo "UID: $(id -u)"
echo "GID: $(id -g)"
echo "Home: $HOME"

check_owner_path "$HOME/.vscode-server"
check_owner_path "$HOME/.vscode-server/extensions"
check_owner_path "$HOME/.vscode-server/extensionsCache"
check_owner_path "$HOME/.vscode-server/data/Machine"

rustc --version
cargo --version
