use corsair_performance_level_core::Level;
use ksni::menu::{CheckmarkItem, MenuItem, StandardItem};

use crate::autostart;
use crate::level_ui::LevelUi;

pub const APP_TITLE: &str = "Corsair Performance";

pub struct Indicator {
    level: Level,
    detail: Option<String>,
    icon_theme_path: String,
    start_automatically: bool,
}

pub struct TrayUpdate {
    pub level: Level,
    pub detail: Option<String>,
}

impl Indicator {
    pub fn new(icon_theme_path: String) -> Self {
        Self {
            level: Level::Unknown,
            detail: Some("Kernel driver may be missing".to_string()),
            icon_theme_path,
            start_automatically: autostart::is_enabled(),
        }
    }

    pub fn apply(&mut self, update: TrayUpdate) {
        self.level = update.level;
        self.detail = update.detail;
    }
}

impl ksni::Tray for Indicator {
    // GNOME/AppIndicator hover tooltips are not dependable, so make primary
    // click open the menu where the title and current level are explicit.
    const MENU_ON_ACTIVATE: bool = true;

    fn id(&self) -> String {
        "corsair-level-indicator".to_string()
    }

    fn category(&self) -> ksni::Category {
        ksni::Category::Hardware
    }

    fn title(&self) -> String {
        APP_TITLE.to_string()
    }

    fn status(&self) -> ksni::Status {
        ksni::Status::Active
    }

    fn icon_theme_path(&self) -> String {
        self.icon_theme_path.clone()
    }

    fn icon_name(&self) -> String {
        self.level.icon_name().to_string()
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        // The title item substitutes for tooltip/app identity in GNOME's
        // indicator menu, while the disabled level row carries live state.
        let mut items = vec![
            StandardItem {
                label: APP_TITLE.to_string(),
                enabled: false,
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: format!("Level: {}", self.level.label()),
                enabled: false,
                ..Default::default()
            }
            .into(),
        ];

        if let Some(detail) = &self.detail {
            items.push(
                StandardItem {
                    label: detail.clone(),
                    enabled: false,
                    ..Default::default()
                }
                .into(),
            );
        }

        items.extend([
            MenuItem::Separator,
            CheckmarkItem {
                label: "Start automatically".to_string(),
                checked: self.start_automatically,
                activate: Box::new(|this: &mut Self| {
                    let enabled = !this.start_automatically;
                    match autostart::set_enabled(enabled) {
                        Ok(()) => {
                            this.start_automatically = enabled;
                        }
                        Err(err) => {
                            this.detail = Some(format!("Cannot update autostart: {err}"));
                        }
                    }
                }),
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            StandardItem {
                label: "Quit".to_string(),
                icon_name: "application-exit-symbolic".to_string(),
                activate: Box::new(|_| std::process::exit(0)),
                ..Default::default()
            }
            .into(),
        ]);

        items
    }
}
