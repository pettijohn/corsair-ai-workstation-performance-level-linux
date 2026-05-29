// SPDX-License-Identifier: GPL-2.0

//! Bind/unbind-only Rust WMI skeleton.
//!
//! This module proves the all-Rust driver can register with the WMI bus and
//! receive lifecycle callbacks. Query, event decoding, and sysfs are later
//! 4.x tasks.

use core::ffi::{c_int, c_void};
use core::mem::MaybeUninit;
use core::ptr;

use kernel::prelude::*;
use kernel::error::to_result;

mod wmi_ffi;

const DRIVER_NAME: &[u8] = b"corsair_wmi\0";

struct WmiIdTable([kernel::bindings::wmi_device_id; 3]);

// The table is immutable after initialization and is only handed to the kernel
// as a C id table pointer.
unsafe impl Sync for WmiIdTable {}

static WMI_IDS: WmiIdTable = WmiIdTable([
    kernel::bindings::wmi_device_id {
        guid_string: wmi_ffi::guid_string(wmi_ffi::EVENT_GUID),
        context: ptr::null(),
    },
    kernel::bindings::wmi_device_id {
        guid_string: wmi_ffi::guid_string(wmi_ffi::METHOD_GUID),
        context: ptr::null(),
    },
    kernel::bindings::wmi_device_id {
        guid_string: [0; 37],
        context: ptr::null(),
    },
]);

static mut WMI_DRIVER: MaybeUninit<wmi_ffi::WmiDriver> = MaybeUninit::uninit();

module! {
    type: CorsairWmiRust,
    name: "corsair_wmi_rust",
    authors: ["Local driver prototype"],
    description: "CORSAIR AI Workstation performance-mode WMI Rust skeleton",
    license: "GPL",
    alias: ["wmi:8FAFC061-22DA-46E2-91DB-1FE3D7E5FF3C", "wmi:99D89064-8D50-42BB-BEA9-155B2E5D0FCD"],
}

struct CorsairWmiRust;

impl kernel::Module for CorsairWmiRust {
    fn init(_module: &'static ThisModule) -> Result<Self> {
        // The Linux WMI subsystem still needs raw C registration. We only set
        // the fields used by `struct wmi_driver`; the embedded driver object is
        // zeroed except for its name.
        let mut driver_model: kernel::bindings::device_driver = unsafe { core::mem::zeroed() };
        driver_model.name = DRIVER_NAME.as_ptr().cast();

        let driver = wmi_ffi::WmiDriver {
            driver: driver_model,
            id_table: WMI_IDS.0.as_ptr(),
            no_notify_data: false,
            no_singleton: true,
            probe: Some(corsair_wmi_probe),
            remove: Some(corsair_wmi_remove),
            shutdown: None,
            notify: None,
            notify_new: Some(corsair_wmi_notify_new),
        };

        let driver_ptr = core::ptr::addr_of_mut!(WMI_DRIVER).cast::<wmi_ffi::WmiDriver>();
        unsafe {
            driver_ptr.write(driver);
            to_result(wmi_ffi::__wmi_driver_register(
                driver_ptr,
                core::ptr::addr_of_mut!(kernel::bindings::__this_module),
            ))?;
        }

        pr_info!("corsair_wmi_rust: registered WMI skeleton\n");
        Ok(Self)
    }
}

impl Drop for CorsairWmiRust {
    fn drop(&mut self) {
        let driver_ptr = core::ptr::addr_of_mut!(WMI_DRIVER).cast::<wmi_ffi::WmiDriver>();

        unsafe {
            wmi_ffi::wmi_driver_unregister(driver_ptr);
        }

        pr_info!("corsair_wmi_rust: unregistered WMI skeleton\n");
    }
}

unsafe extern "C" fn corsair_wmi_probe(
    _wdev: *mut wmi_ffi::WmiDevice,
    _context: *const c_void,
) -> c_int {
    pr_info!("corsair_wmi_rust: probe callback\n");
    0
}

unsafe extern "C" fn corsair_wmi_remove(_wdev: *mut wmi_ffi::WmiDevice) {
    pr_info!("corsair_wmi_rust: remove callback\n");
}

unsafe extern "C" fn corsair_wmi_notify_new(
    _wdev: *mut wmi_ffi::WmiDevice,
    data: *const wmi_ffi::WmiBuffer,
) {
    let len = if data.is_null() {
        0
    } else {
        // Callback ownership stays with the WMI core; for the skeleton we only
        // inspect the buffer length to prove notification plumbing.
        unsafe { (*data).length }
    };

    pr_info!("corsair_wmi_rust: notify_new callback len={}\n", len);
}
