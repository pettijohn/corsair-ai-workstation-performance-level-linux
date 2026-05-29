// SPDX-License-Identifier: GPL-2.0

//! Mode decoding used by the Rust kernel driver.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub(crate) enum Mode {
    Balanced = 0,
    Max = 1,
    Quiet = 2,
    Super = 3,
    Unknown = 255,
}

impl Mode {
    pub(crate) const fn from_raw(raw: u8) -> Self {
        match raw {
            0 => Self::Balanced,
            1 => Self::Max,
            2 => Self::Quiet,
            3 => Self::Super,
            _ => Self::Unknown,
        }
    }

    pub(crate) const fn from_query_value(value: u64) -> Self {
        match value {
            0 => Self::Balanced,
            1 => Self::Max,
            2 => Self::Quiet,
            3 => Self::Super,
            _ => Self::Unknown,
        }
    }

    pub(crate) const fn from_event_detail(detail: u8) -> Self {
        match detail {
            0x11 => Self::Quiet,
            0x12 => Self::Balanced,
            0x13 => Self::Max,
            0x14 => Self::Super,
            _ => Self::Unknown,
        }
    }

    pub(crate) const fn raw_value(self) -> u8 {
        self as u8
    }

    pub(crate) const fn name_cstr(self) -> *const u8 {
        match self {
            Self::Quiet => b"quiet\0".as_ptr(),
            Self::Balanced => b"balanced\0".as_ptr(),
            Self::Max => b"max\0".as_ptr(),
            Self::Super => b"super\0".as_ptr(),
            Self::Unknown => b"unknown\0".as_ptr(),
        }
    }

    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Quiet => "quiet",
            Self::Balanced => "balanced",
            Self::Max => "max",
            Self::Super => "super",
            Self::Unknown => "unknown",
        }
    }
}

pub(crate) const fn is_selector_event(payload: &[u8]) -> bool {
    if payload.len() < 3 {
        return false;
    }

    payload[0] == 0x01 && payload[2] == 0x81 && payload[1] >= 0x11 && payload[1] <= 0x14
}
