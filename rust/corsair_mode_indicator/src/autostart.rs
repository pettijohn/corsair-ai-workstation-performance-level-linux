use std::fs;
use std::io;
use std::path::PathBuf;

const DESKTOP_FILE_NAME: &str = "corsair-mode-indicator.desktop";

pub fn is_enabled() -> bool {
    let Ok(contents) = fs::read_to_string(autostart_file()) else {
        return false;
    };

    contents
        .lines()
        .find_map(|line| {
            let (key, value) = line.split_once('=')?;
            key.eq_ignore_ascii_case("X-GNOME-Autostart-enabled")
                .then(|| value.trim().eq_ignore_ascii_case("true"))
        })
        .unwrap_or(false)
}

pub fn set_enabled(enabled: bool) -> io::Result<()> {
    let path = autostart_file();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let exec = std::env::current_exe()?;
    fs::write(
        path,
        format!(
            "[Desktop Entry]\n\
             Type=Application\n\
             Name=Corsair Performance\n\
             Comment=Show the CORSAIR AI Workstation performance mode\n\
             Exec={}\n\
             Icon=corsair-mode-balanced-symbolic\n\
             Terminal=false\n\
             Categories=System;HardwareSettings;\n\
             X-GNOME-Autostart-enabled={}\n",
            desktop_exec_value(&exec.display().to_string()),
            if enabled { "true" } else { "false" }
        ),
    )
}

fn desktop_exec_value(path: &str) -> String {
    if path.contains(char::is_whitespace) {
        format!("\"{}\"", path.replace('\\', "\\\\").replace('"', "\\\""))
    } else {
        path.to_string()
    }
}

fn autostart_file() -> PathBuf {
    config_home().join("autostart").join(DESKTOP_FILE_NAME)
}

fn config_home() -> PathBuf {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
        .unwrap_or_else(|| PathBuf::from(".config"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_when_no_autostart_file_exists() {
        let unique = format!("corsair-mode-indicator-test-{}", std::process::id());
        let path = std::env::temp_dir().join(unique);
        std::env::set_var("XDG_CONFIG_HOME", &path);

        assert!(!is_enabled());

        let _ = fs::remove_dir_all(path);
    }

    #[test]
    fn quotes_exec_paths_with_spaces() {
        assert_eq!(
            desktop_exec_value("/home/me/Corsair App/corsair-mode-indicator"),
            "\"/home/me/Corsair App/corsair-mode-indicator\""
        );
        assert_eq!(
            desktop_exec_value("/home/me/.local/bin/corsair-mode-indicator"),
            "/home/me/.local/bin/corsair-mode-indicator"
        );
    }
}
