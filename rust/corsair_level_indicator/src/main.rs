mod autostart;
mod indicator;
mod level_ui;
mod watcher;

use std::path::PathBuf;
use std::sync::mpsc;

use indicator::{Indicator, TrayUpdate};
use ksni::blocking::TrayMethods;
use watcher::{spawn_watcher, WatchEvent, DEFAULT_CURRENT_LEVEL_PATH};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // AppIndicator icon-theme discovery can lag or miss per-user installs, so
    // advertise our installed status-icon directory directly. The env override
    // keeps uninstalled repo-local runs useful during development.
    let icon_theme_path = icon_theme_path();
    let indicator = Indicator::new(icon_theme_path);
    let handle = indicator
        .assume_sni_available(true)
        .spawn()
        .map_err(|err| format!("failed to start StatusNotifierItem service: {err}"))?;

    // Keep sysfs blocking work off the SNI service thread; tray state updates
    // are funneled back through the ksni handle.
    let (tx, rx) = mpsc::channel();
    let path = std::env::var_os("CORSAIR_LEVEL_SYSFS_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(DEFAULT_CURRENT_LEVEL_PATH));
    let _watcher = spawn_watcher(path, tx);

    for event in rx {
        let update = match event {
            WatchEvent::Level(level) => TrayUpdate {
                level,
                detail: None,
            },
            WatchEvent::Unavailable(detail) => TrayUpdate {
                level: corsair_performance_level_core::Level::Unknown,
                detail: Some(detail),
            },
        };

        if handle.update(|tray| tray.apply(update)).is_none() {
            break;
        }
    }

    Ok(())
}

fn icon_theme_path() -> String {
    if let Ok(path) = std::env::var("CORSAIR_LEVEL_ICON_THEME_PATH") {
        return path;
    }

    let path = xdg_data_home()
        .join("icons")
        .join("hicolor")
        .join("scalable")
        .join("status");
    if path.is_dir() {
        path.display().to_string()
    } else {
        String::new()
    }
}

fn xdg_data_home() -> PathBuf {
    std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/share")))
        .unwrap_or_else(|| PathBuf::from(".local/share"))
}
