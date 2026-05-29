#![cfg_attr(not(test), no_std)]

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum Level {
    Balanced = 0,
    Max = 1,
    Quiet = 2,
    Super = 3,
    Unknown = 255,
}

impl Level {
    pub const fn from_query_value(value: u8) -> Self {
        match value {
            0 => Self::Balanced,
            1 => Self::Max,
            2 => Self::Quiet,
            3 => Self::Super,
            _ => Self::Unknown,
        }
    }

    pub const fn from_event_detail(detail: u8) -> Self {
        match detail {
            0x11 => Self::Quiet,
            0x12 => Self::Balanced,
            0x13 => Self::Max,
            0x14 => Self::Super,
            _ => Self::Unknown,
        }
    }

    pub const fn raw_value(self) -> u8 {
        self as u8
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Quiet => "quiet",
            Self::Balanced => "balanced",
            Self::Max => "max",
            Self::Super => "super",
            Self::Unknown => "unknown",
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Quiet => "Quiet",
            Self::Balanced => "Balanced",
            Self::Max => "Max",
            Self::Super => "Super",
            Self::Unknown => "Unknown",
        }
    }

    pub fn from_sysfs_value(value: &str) -> Self {
        match value.trim() {
            "quiet" => Self::Quiet,
            "balanced" => Self::Balanced,
            "max" => Self::Max,
            "super" => Self::Super,
            "unknown" => Self::Unknown,
            _ => Self::Unknown,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SelectorEvent {
    pub level: Level,
    pub raw_detail: u8,
}

pub const fn is_selector_event(payload: &[u8]) -> bool {
    if payload.len() < 3 {
        return false;
    }

    payload[0] == 0x01 && payload[2] == 0x81 && payload[1] >= 0x11 && payload[1] <= 0x14
}

pub const fn decode_selector_event(payload: &[u8]) -> Option<SelectorEvent> {
    if !is_selector_event(payload) {
        return None;
    }

    Some(SelectorEvent {
        level: Level::from_event_detail(payload[1]),
        raw_detail: payload[1],
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_current_level_query_values() {
        assert_eq!(Level::from_query_value(0), Level::Balanced);
        assert_eq!(Level::from_query_value(1), Level::Max);
        assert_eq!(Level::from_query_value(2), Level::Quiet);
        assert_eq!(Level::from_query_value(3), Level::Super);
        assert_eq!(Level::from_query_value(0xff), Level::Unknown);
    }

    #[test]
    fn exposes_stable_sysfs_names_and_raw_values() {
        assert_eq!(Level::Quiet.as_str(), "quiet");
        assert_eq!(Level::Balanced.as_str(), "balanced");
        assert_eq!(Level::Max.as_str(), "max");
        assert_eq!(Level::Super.as_str(), "super");
        assert_eq!(Level::Unknown.as_str(), "unknown");

        assert_eq!(Level::Quiet.label(), "Quiet");
        assert_eq!(Level::Balanced.label(), "Balanced");
        assert_eq!(Level::Max.label(), "Max");
        assert_eq!(Level::Super.label(), "Super");
        assert_eq!(Level::Unknown.label(), "Unknown");

        assert_eq!(Level::Balanced.raw_value(), 0);
        assert_eq!(Level::Max.raw_value(), 1);
        assert_eq!(Level::Quiet.raw_value(), 2);
        assert_eq!(Level::Super.raw_value(), 3);
        assert_eq!(Level::Unknown.raw_value(), 255);
    }

    #[test]
    fn parses_sysfs_values() {
        assert_eq!(Level::from_sysfs_value("quiet\n"), Level::Quiet);
        assert_eq!(Level::from_sysfs_value("balanced\n"), Level::Balanced);
        assert_eq!(Level::from_sysfs_value("max\n"), Level::Max);
        assert_eq!(Level::from_sysfs_value("super\n"), Level::Super);
        assert_eq!(Level::from_sysfs_value("unknown\n"), Level::Unknown);
        assert_eq!(Level::from_sysfs_value("turbo\n"), Level::Unknown);
    }

    #[test]
    fn decodes_selector_events() {
        assert_eq!(
            decode_selector_event(&[0x01, 0x11, 0x81, 0, 0, 0, 0, 0]),
            Some(SelectorEvent {
                level: Level::Quiet,
                raw_detail: 0x11,
            })
        );
        assert_eq!(
            decode_selector_event(&[0x01, 0x12, 0x81, 0, 0, 0, 0, 0]).map(|event| event.level),
            Some(Level::Balanced)
        );
        assert_eq!(
            decode_selector_event(&[0x01, 0x13, 0x81, 0, 0, 0, 0, 0]).map(|event| event.level),
            Some(Level::Max)
        );
        assert_eq!(
            decode_selector_event(&[0x01, 0x14, 0x81, 0, 0, 0, 0, 0]).map(|event| event.level),
            Some(Level::Super)
        );
    }

    #[test]
    fn rejects_non_selector_events() {
        assert!(!is_selector_event(&[]));
        assert!(!is_selector_event(&[0x01, 0x11]));
        assert!(!is_selector_event(&[0x00, 0x11, 0x81]));
        assert!(!is_selector_event(&[0x01, 0x0a, 0x81, 0, 0, 0, 0, 0]));
        assert!(!is_selector_event(&[0x01, 0x11, 0x80, 0, 0, 0, 0, 0]));
        assert_eq!(
            decode_selector_event(&[0x01, 0x0a, 0x81, 0, 0, 0, 0, 0]),
            None
        );
    }
}
