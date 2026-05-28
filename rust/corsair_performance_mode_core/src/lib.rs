#![cfg_attr(not(test), no_std)]

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum Mode {
    Balanced = 0,
    Max = 1,
    Quiet = 2,
    Super = 3,
    Unknown = 255,
}

impl Mode {
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
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SelectorEvent {
    pub mode: Mode,
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
        mode: Mode::from_event_detail(payload[1]),
        raw_detail: payload[1],
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_current_mode_query_values() {
        assert_eq!(Mode::from_query_value(0), Mode::Balanced);
        assert_eq!(Mode::from_query_value(1), Mode::Max);
        assert_eq!(Mode::from_query_value(2), Mode::Quiet);
        assert_eq!(Mode::from_query_value(3), Mode::Super);
        assert_eq!(Mode::from_query_value(0xff), Mode::Unknown);
    }

    #[test]
    fn exposes_stable_sysfs_names_and_raw_values() {
        assert_eq!(Mode::Quiet.as_str(), "quiet");
        assert_eq!(Mode::Balanced.as_str(), "balanced");
        assert_eq!(Mode::Max.as_str(), "max");
        assert_eq!(Mode::Super.as_str(), "super");
        assert_eq!(Mode::Unknown.as_str(), "unknown");

        assert_eq!(Mode::Balanced.raw_value(), 0);
        assert_eq!(Mode::Max.raw_value(), 1);
        assert_eq!(Mode::Quiet.raw_value(), 2);
        assert_eq!(Mode::Super.raw_value(), 3);
        assert_eq!(Mode::Unknown.raw_value(), 255);
    }

    #[test]
    fn decodes_selector_events() {
        assert_eq!(
            decode_selector_event(&[0x01, 0x11, 0x81, 0, 0, 0, 0, 0]),
            Some(SelectorEvent {
                mode: Mode::Quiet,
                raw_detail: 0x11,
            })
        );
        assert_eq!(
            decode_selector_event(&[0x01, 0x12, 0x81, 0, 0, 0, 0, 0]).map(|event| event.mode),
            Some(Mode::Balanced)
        );
        assert_eq!(
            decode_selector_event(&[0x01, 0x13, 0x81, 0, 0, 0, 0, 0]).map(|event| event.mode),
            Some(Mode::Max)
        );
        assert_eq!(
            decode_selector_event(&[0x01, 0x14, 0x81, 0, 0, 0, 0, 0]).map(|event| event.mode),
            Some(Mode::Super)
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
