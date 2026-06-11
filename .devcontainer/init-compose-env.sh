#!/usr/bin/env bash

set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
env_file="${script_dir}/.env"

user_uid=1000
user_gid=1000
devcontainer_id="$(basename "$(dirname "${script_dir}")")"

if command -v id >/dev/null 2>&1; then
  user_uid="$(id -u)"
  user_gid="$(id -g)"
fi

if [ -f "${env_file}" ]; then
  if ! grep -q '^DEVCONTAINER_ID=' "${env_file}"; then
    printf '\nDEVCONTAINER_ID=%s\n' "${devcontainer_id}" >>"${env_file}"
  fi
  exit 0
fi

cat >"${env_file}" <<EOF
DEVCONTAINER_ID=${devcontainer_id}
USER_UID=${user_uid}
USER_GID=${user_gid}
EOF
