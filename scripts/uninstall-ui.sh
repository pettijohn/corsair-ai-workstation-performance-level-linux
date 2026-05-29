#!/usr/bin/env sh
set -eu

rm -f "${HOME}/.local/bin/corsair-level-indicator"
rm -f "${HOME}/.local/bin/corsair-mode-indicator"
rm -f "${HOME}/.local/share/icons/hicolor/scalable/status"/corsair-level-*-symbolic.svg
rm -f "${HOME}/.local/share/icons/hicolor/scalable/status"/corsair-mode-*-symbolic.svg
rm -f "${HOME}/.local/share/applications/corsair-level-indicator.desktop"
rm -f "${HOME}/.local/share/applications/corsair-mode-indicator.desktop"
rm -f "${HOME}/.config/autostart/corsair-level-indicator.desktop"
rm -f "${HOME}/.config/autostart/corsair-mode-indicator.desktop"

if command -v gtk-update-icon-cache >/dev/null 2>&1; then
    gtk-update-icon-cache -q "${HOME}/.local/share/icons/hicolor" || true
fi
if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database -q "${HOME}/.local/share/applications" || true
fi

echo "Removed Corsair Performance indicator."
