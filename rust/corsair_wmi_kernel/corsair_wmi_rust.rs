// SPDX-License-Identifier: GPL-2.0

//! Rust WMI driver for the CORSAIR AI Workstation performance selector.
//!
//! This driver is read-only: it queries method id 2 for the current mode,
//! decodes selector events, and exposes the cached mode through sysfs.

use core::ffi::{c_int, c_void};
use core::mem::MaybeUninit;
use core::ptr;
use core::slice;
use core::sync::atomic::{AtomicPtr, AtomicU8, Ordering};

use kernel::error::to_result;
use kernel::prelude::*;

mod mode;
mod wmi_ffi;

const DRIVER_NAME: &[u8] = b"corsair_wmi\0";
const METHOD_ID_CURRENT_MODE: u32 = 2;

const CURRENT_MODE_ATTR_NAME: &[u8] = b"current_mode\0";
const CURRENT_MODE_RAW_ATTR_NAME: &[u8] = b"current_mode_raw\0";

struct WmiIdTable([kernel::bindings::wmi_device_id; 3]);
struct DeviceAttr(kernel::bindings::device_attribute);

// The table is immutable after initialization and is only handed to the kernel
// as a C id table pointer.
unsafe impl Sync for WmiIdTable {}
unsafe impl Sync for DeviceAttr {}

static WMI_IDS: WmiIdTable = WmiIdTable([
    kernel::bindings::wmi_device_id {
        guid_string: wmi_ffi::guid_string(wmi_ffi::EVENT_GUID),
        context: wmi_ffi::EVENT_CONTEXT,
    },
    kernel::bindings::wmi_device_id {
        guid_string: wmi_ffi::guid_string(wmi_ffi::METHOD_GUID),
        context: wmi_ffi::METHOD_CONTEXT,
    },
    kernel::bindings::wmi_device_id {
        guid_string: [0; 37],
        context: ptr::null(),
    },
]);

static mut WMI_DRIVER: MaybeUninit<wmi_ffi::WmiDriver> = MaybeUninit::uninit();
static METHOD_WDEV: AtomicPtr<wmi_ffi::WmiDevice> = AtomicPtr::new(ptr::null_mut());
static CURRENT_MODE: AtomicU8 = AtomicU8::new(mode::Mode::Unknown as u8);

static CURRENT_MODE_ATTR: DeviceAttr = DeviceAttr(wmi_ffi::read_only_attr(
    CURRENT_MODE_ATTR_NAME.as_ptr(),
    current_mode_show,
));
static CURRENT_MODE_RAW_ATTR: DeviceAttr = DeviceAttr(wmi_ffi::read_only_attr(
    CURRENT_MODE_RAW_ATTR_NAME.as_ptr(),
    current_mode_raw_show,
));

module! {
    type: CorsairWmiRust,
    name: "corsair_wmi_rust",
    authors: ["Local driver prototype"],
    description: "Read-only CORSAIR AI Workstation performance-mode WMI Rust driver",
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

        pr_info!("corsair_wmi: registered Rust WMI driver\n");
        Ok(Self)
    }
}

impl Drop for CorsairWmiRust {
    fn drop(&mut self) {
        let driver_ptr = core::ptr::addr_of_mut!(WMI_DRIVER).cast::<wmi_ffi::WmiDriver>();

        unsafe {
            wmi_ffi::wmi_driver_unregister(driver_ptr);
        }

        pr_info!("corsair_wmi: unregistered Rust WMI driver\n");
    }
}

unsafe extern "C" fn corsair_wmi_probe(
    wdev: *mut wmi_ffi::WmiDevice,
    context: *const c_void,
) -> c_int {
    pr_info!("corsair_wmi: probe callback\n");

    if context == wmi_ffi::METHOD_CONTEXT {
        METHOD_WDEV.store(wdev, Ordering::Release);

        let ret = unsafe { create_mode_attrs(wdev) };
        if ret != 0 {
            METHOD_WDEV.store(ptr::null_mut(), Ordering::Release);
            return ret;
        }

        if let Err(ret) = unsafe { query_current_mode(wdev) } {
            pr_info!("corsair_wmi: initial mode query failed ret={}\n", ret);
        }
    }

    0
}

unsafe extern "C" fn corsair_wmi_remove(wdev: *mut wmi_ffi::WmiDevice) {
    if METHOD_WDEV.load(Ordering::Acquire) == wdev {
        unsafe {
            remove_mode_attrs(wdev);
        }
        METHOD_WDEV.store(ptr::null_mut(), Ordering::Release);
    }

    pr_info!("corsair_wmi: remove callback\n");
}

unsafe extern "C" fn corsair_wmi_notify_new(
    _wdev: *mut wmi_ffi::WmiDevice,
    data: *const wmi_ffi::WmiBuffer,
) {
    if data.is_null() || unsafe { (*data).data.is_null() } {
        return;
    }

    let payload = unsafe { slice::from_raw_parts((*data).data.cast::<u8>(), (*data).length) };
    if !mode::is_selector_event(payload) {
        return;
    }

    let detail = payload[1];
    let mode = mode::Mode::from_event_detail(detail);

    pr_info!(
        "corsair_wmi: selector event detail=0x{:02x} mode_raw={}\n",
        detail,
        mode.raw_value()
    );
    set_cached_mode(mode, "event");
}

unsafe fn create_mode_attrs(wdev: *mut wmi_ffi::WmiDevice) -> c_int {
    let dev = unsafe { core::ptr::addr_of_mut!((*wdev).dev) };
    let ret =
        unsafe { wmi_ffi::device_create_file(dev, core::ptr::addr_of!(CURRENT_MODE_ATTR.0)) };
    if ret != 0 {
        return ret;
    }

    let ret =
        unsafe { wmi_ffi::device_create_file(dev, core::ptr::addr_of!(CURRENT_MODE_RAW_ATTR.0)) };
    if ret != 0 {
        unsafe {
            wmi_ffi::device_remove_file(dev, core::ptr::addr_of!(CURRENT_MODE_ATTR.0));
        }
    }

    ret
}

unsafe fn remove_mode_attrs(wdev: *mut wmi_ffi::WmiDevice) {
    let dev = unsafe { core::ptr::addr_of_mut!((*wdev).dev) };

    unsafe {
        wmi_ffi::device_remove_file(dev, core::ptr::addr_of!(CURRENT_MODE_RAW_ATTR.0));
        wmi_ffi::device_remove_file(dev, core::ptr::addr_of!(CURRENT_MODE_ATTR.0));
    }
}

unsafe fn query_current_mode(wdev: *mut wmi_ffi::WmiDevice) -> core::result::Result<(), c_int> {
    let input = kernel::bindings::acpi_buffer {
        length: 0,
        pointer: ptr::null_mut(),
    };
    let mut output = kernel::bindings::acpi_buffer {
        length: wmi_ffi::ACPI_ALLOCATE_BUFFER,
        pointer: ptr::null_mut(),
    };

    let status = unsafe {
        wmi_ffi::wmidev_evaluate_method(
            wdev,
            0,
            METHOD_ID_CURRENT_MODE,
            core::ptr::addr_of!(input),
            core::ptr::addr_of_mut!(output),
        )
    };
    if status != 0 {
        return Err(-(kernel::bindings::EIO as c_int));
    }

    let obj = output.pointer.cast::<kernel::bindings::acpi_object>();
    if obj.is_null() {
        return Err(-(kernel::bindings::ENODATA as c_int));
    }

    let object_type = unsafe { (*obj).type_ };
    if object_type != kernel::bindings::ACPI_TYPE_INTEGER {
        unsafe {
            wmi_ffi::kfree(output.pointer);
        }
        return Err(-(kernel::bindings::ENODATA as c_int));
    }

    let value = unsafe { (*obj).integer.value };
    let mode = mode::Mode::from_query_value(value);
    unsafe {
        wmi_ffi::kfree(output.pointer);
    }

    set_cached_mode(mode, "query");
    Ok(())
}

fn set_cached_mode(mode: mode::Mode, source: &'static str) {
    let old = CURRENT_MODE.swap(mode.raw_value(), Ordering::AcqRel);

    if old == mode.raw_value() {
        pr_info!(
            "corsair_wmi: mode={} raw={} source={} unchanged\n",
            mode.as_str(),
            mode.raw_value(),
            source
        );
    } else {
        pr_info!(
            "corsair_wmi: mode={} raw={} source={}\n",
            mode.as_str(),
            mode.raw_value(),
            source
        );
    }

    if old != mode.raw_value() {
        notify_mode_attrs();
    }
}

fn notify_mode_attrs() {
    let wdev = METHOD_WDEV.load(Ordering::Acquire);
    if wdev.is_null() {
        return;
    }

    unsafe {
        let kobj = core::ptr::addr_of_mut!((*wdev).dev.kobj);
        wmi_ffi::sysfs_notify(kobj, ptr::null(), CURRENT_MODE_ATTR_NAME.as_ptr());
        wmi_ffi::sysfs_notify(kobj, ptr::null(), CURRENT_MODE_RAW_ATTR_NAME.as_ptr());
    }
}

unsafe extern "C" fn current_mode_show(
    _dev: *mut kernel::bindings::device,
    _attr: *mut kernel::bindings::device_attribute,
    buf: *mut u8,
) -> isize {
    let mode = mode::Mode::from_raw(CURRENT_MODE.load(Ordering::Acquire));

    unsafe { wmi_ffi::sysfs_emit(buf, b"%s\n\0".as_ptr(), mode.name_cstr()) as isize }
}

unsafe extern "C" fn current_mode_raw_show(
    _dev: *mut kernel::bindings::device,
    _attr: *mut kernel::bindings::device_attribute,
    buf: *mut u8,
) -> isize {
    let mode = CURRENT_MODE.load(Ordering::Acquire);

    unsafe { wmi_ffi::sysfs_emit(buf, b"%u\n\0".as_ptr(), wmi_ffi::u32_arg(mode)) as isize }
}
