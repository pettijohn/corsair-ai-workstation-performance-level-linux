// SPDX-License-Identifier: GPL-2.0

//! Minimal Linux WMI FFI for the Rust kernel prototype.
//!
//! The Ubuntu 7.0 Rust bindings include generic driver-model and ACPI types,
//! but they do not expose `struct wmi_driver` or the WMI registration helpers.
//! Keep this file deliberately small and mirror `include/linux/wmi.h`.

#![allow(dead_code, improper_ctypes)]

use core::ffi::{c_char, c_int, c_void};

use kernel::bindings;

pub(crate) const METHOD_GUID: &[u8; 37] = b"99D89064-8D50-42BB-BEA9-155B2E5D0FCD\0";
pub(crate) const EVENT_GUID: &[u8; 37] = b"8FAFC061-22DA-46E2-91DB-1FE3D7E5FF3C\0";

/// WMI devices begin with `struct device`; the remaining fields are from
/// `include/linux/wmi.h`.
#[repr(C)]
pub(crate) struct WmiDevice {
    pub dev: bindings::device,
    pub setable: bindings::bool_,
    pub driver_override: *const c_char,
}

/// Buffer passed to modern WMI event callbacks.
#[repr(C)]
pub(crate) struct WmiBuffer {
    pub length: usize,
    pub data: *mut c_void,
}

/// Driver registration object consumed by the Linux WMI core.
#[repr(C)]
pub(crate) struct WmiDriver {
    pub driver: bindings::device_driver,
    pub id_table: *const bindings::wmi_device_id,
    pub no_notify_data: bool,
    pub no_singleton: bool,
    pub probe: Option<unsafe extern "C" fn(*mut WmiDevice, *const c_void) -> c_int>,
    pub remove: Option<unsafe extern "C" fn(*mut WmiDevice)>,
    pub shutdown: Option<unsafe extern "C" fn(*mut WmiDevice)>,
    pub notify: Option<unsafe extern "C" fn(*mut WmiDevice, *mut bindings::acpi_object)>,
    pub notify_new: Option<unsafe extern "C" fn(*mut WmiDevice, *const WmiBuffer)>,
}

pub(crate) const fn guid_string(bytes: &[u8; 37]) -> [u8; 37] {
    let mut out = [0; 37];
    let mut i = 0;

    while i < 37 {
        out[i] = bytes[i];
        i += 1;
    }

    out
}

unsafe extern "C" {
    pub(crate) fn __wmi_driver_register(
        driver: *mut WmiDriver,
        owner: *mut bindings::module,
    ) -> c_int;
    pub(crate) fn wmi_driver_unregister(driver: *mut WmiDriver);

    pub(crate) fn wmidev_evaluate_method(
        wdev: *mut WmiDevice,
        instance: u8,
        method_id: u32,
        input: *const bindings::acpi_buffer,
        output: *mut bindings::acpi_buffer,
    ) -> bindings::acpi_status;
}
