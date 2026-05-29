#!/usr/bin/env sh
set -eu

rm -f "${HOME}/.local/bin/corsair-mode-indicator"
rm -f "${HOME}/.local/share/icons/hicolor/scalable/status"/corsair-mode-*-symbolic.svg
rm -f "${HOME}/.local/share/applications/corsair-mode-indicator.desktop"
rm -f "${HOME}/.config/autostart/corsair-mode-indicator.desktop"

if command -v gtk-update-icon-cache >/dev/null 2>&1; then
    gtk-update-icon-cache -q "${HOME}/.local/share/icons/hicolor" || true
fi

echo "Removed Corsair Performance indicator."
