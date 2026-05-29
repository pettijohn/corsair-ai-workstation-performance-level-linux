// SPDX-License-Identifier: GPL-2.0

//! Minimal Linux WMI FFI for the Rust kernel prototype.
//!
//! The Ubuntu 7.0 Rust bindings include generic driver-model and ACPI types,
//! but they do not expose `struct wmi_driver` or the WMI registration helpers.
//! Keep this file deliberately small and mirror `include/linux/wmi.h`.

#![allow(dead_code, improper_ctypes)]

use core::ffi::{c_int, c_uint, c_void};

use kernel::bindings;

pub(crate) const METHOD_GUID: &[u8; 37] = b"99D89064-8D50-42BB-BEA9-155B2E5D0FCD\0";
pub(crate) const EVENT_GUID: &[u8; 37] = b"8FAFC061-22DA-46E2-91DB-1FE3D7E5FF3C\0";

pub(crate) const ACPI_ALLOCATE_BUFFER: bindings::acpi_size = !0;
pub(crate) const METHOD_CONTEXT: *const c_void = 1usize as *const c_void;
pub(crate) const EVENT_CONTEXT: *const c_void = 2usize as *const c_void;

/// WMI devices begin with `struct device`; the remaining fields are from
/// `include/linux/wmi.h`.
#[repr(C)]
pub(crate) struct WmiDevice {
    pub dev: bindings::device,
    pub setable: bindings::bool_,
    pub driver_override: *const u8,
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

    pub(crate) fn device_create_file(
        device: *mut bindings::device,
        entry: *const bindings::device_attribute,
    ) -> c_int;
    pub(crate) fn device_remove_file(
        device: *mut bindings::device,
        entry: *const bindings::device_attribute,
    );
    pub(crate) fn sysfs_notify(
        kobj: *mut bindings::kobject,
        dir: *const u8,
        attr: *const u8,
    );
    pub(crate) fn sysfs_emit(buf: *mut u8, fmt: *const u8, ...) -> c_int;
    pub(crate) fn kfree(objp: *const c_void);
}

pub(crate) const fn read_only_attr(
    name: *const u8,
    show: unsafe extern "C" fn(
        dev: *mut bindings::device,
        attr: *mut bindings::device_attribute,
        buf: *mut u8,
    ) -> isize,
) -> bindings::device_attribute {
    bindings::device_attribute {
        attr: bindings::attribute {
            name,
            mode: bindings::S_IRUGO as bindings::umode_t,
        },
        show: Some(show),
        store: None,
    }
}

pub(crate) fn u32_arg(value: u8) -> c_uint {
    value as c_uint
}
