use corsair_performance_level_core::Level;

pub trait LevelUi {
    fn icon_name(self) -> &'static str;
}

impl LevelUi for Level {
    fn icon_name(self) -> &'static str {
        // Keep every firmware-visible state distinct, including Super, so a
        // future firmware that enables it does not need UI model changes.
        match self {
            Level::Quiet => "corsair-level-quiet-symbolic",
            Level::Balanced => "corsair-level-balanced-symbolic",
            Level::Max => "corsair-level-max-symbolic",
            Level::Super => "corsair-level-super-symbolic",
            Level::Unknown => "corsair-level-unknown-symbolic",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_max_and_super_icons_separate() {
        assert_eq!(Level::Max.icon_name(), "corsair-level-max-symbolic");
        assert_eq!(Level::Super.icon_name(), "corsair-level-super-symbolic");
    }
}
