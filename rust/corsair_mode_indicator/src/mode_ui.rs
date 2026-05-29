use corsair_performance_mode_core::Mode;

pub trait ModeUi {
    fn icon_name(self) -> &'static str;
}

impl ModeUi for Mode {
    fn icon_name(self) -> &'static str {
        // Keep every firmware-visible state distinct, including Super, so a
        // future firmware that enables it does not need UI model changes.
        match self {
            Mode::Quiet => "corsair-mode-quiet-symbolic",
            Mode::Balanced => "corsair-mode-balanced-symbolic",
            Mode::Max => "corsair-mode-max-symbolic",
            Mode::Super => "corsair-mode-super-symbolic",
            Mode::Unknown => "corsair-mode-unknown-symbolic",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_max_and_super_icons_separate() {
        assert_eq!(Mode::Max.icon_name(), "corsair-mode-max-symbolic");
        assert_eq!(Mode::Super.icon_name(), "corsair-mode-super-symbolic");
    }
}
