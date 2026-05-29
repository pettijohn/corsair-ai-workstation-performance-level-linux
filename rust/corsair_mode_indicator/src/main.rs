mod autostart;
mod indicator;
mod mode_ui;
mod watcher;

use std::path::PathBuf;
use std::sync::mpsc;

use indicator::{Indicator, TrayUpdate};
use ksni::blocking::TrayMethods;
use watcher::{spawn_watcher, WatchEvent, DEFAULT_CURRENT_MODE_PATH};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Installed icons come from the user's icon theme; this override keeps
    // uninstalled repo-local runs useful during development.
    let icon_theme_path = std::env::var("CORSAIR_MODE_ICON_THEME_PATH").unwrap_or_default();
    let indicator = Indicator::new(icon_theme_path);
    let handle = indicator
        .assume_sni_available(true)
        .spawn()
        .map_err(|err| format!("failed to start StatusNotifierItem service: {err}"))?;

    // Keep sysfs blocking work off the SNI service thread; tray state updates
    // are funneled back through the ksni handle.
    let (tx, rx) = mpsc::channel();
    let path = std::env::var_os("CORSAIR_MODE_SYSFS_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(DEFAULT_CURRENT_MODE_PATH));
    let _watcher = spawn_watcher(path, tx);

    for event in rx {
        let update = match event {
            WatchEvent::Mode(mode) => TrayUpdate { mode, detail: None },
            WatchEvent::Unavailable(detail) => TrayUpdate {
                mode: corsair_performance_mode_core::Mode::Unknown,
                detail: Some(detail),
            },
        };

        if handle.update(|tray| tray.apply(update)).is_none() {
            break;
        }
    }

    Ok(())
}
