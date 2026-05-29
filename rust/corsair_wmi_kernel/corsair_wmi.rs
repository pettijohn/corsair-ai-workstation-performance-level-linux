// SPDX-License-Identifier: GPL-2.0

//! Rust WMI driver for the CORSAIR AI Workstation performance level selector.
//!
//! This driver is read-only: it queries method id 2 for the current level,
//! decodes selector events, and exposes the cached level through sysfs.

use core::ffi::{c_int, c_void};
use core::mem::MaybeUninit;
use core::ptr;
use core::sync::atomic::{AtomicU8, Ordering};

use kernel::error::to_result;
use kernel::prelude::*;

mod level;
mod wmi_ffi;

const DRIVER_NAME: &[u8] = b"corsair_wmi\0";
const METHOD_ID_CURRENT_LEVEL: u32 = 2;

const CURRENT_LEVEL_ATTR_NAME: &[u8] = b"current_level\0";
const CURRENT_LEVEL_RAW_ATTR_NAME: &[u8] = b"current_level_raw\0";

struct WmiIdTable([kernel::bindings::wmi_device_id; 3]);
struct DeviceAttr(kernel::bindings::device_attribute);

// These wrappers make immutable C tables usable as Rust statics. The WMI core
// only reads the id table, and sysfs only reads the device_attribute metadata.
unsafe impl Sync for WmiIdTable {}
unsafe impl Sync for DeviceAttr {}

// The context values let the shared probe callback tell the method and event
// devices apart without string-comparing dev_name().
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

// The WMI core expects a stable `struct wmi_driver` address for the lifetime of
// the module, so the registration object lives in static storage.
static mut WMI_DRIVER: MaybeUninit<wmi_ffi::WmiDriver> = MaybeUninit::uninit();

// The method WMI device owns the sysfs files. Selector events arrive on the
// event WMI device but notify userspace through this cached method-device kobj.
// METHOD_DEV is protected by METHOD_DEV_LOCK and holds a get_device() reference
// while non-null.
static mut METHOD_DEV_LOCK: MaybeUninit<kernel::bindings::mutex> = MaybeUninit::uninit();
static mut METHOD_DEV: *mut kernel::bindings::device = ptr::null_mut();
static CURRENT_LEVEL: AtomicU8 = AtomicU8::new(level::Level::Unknown as u8);

static CURRENT_LEVEL_ATTR: DeviceAttr = DeviceAttr(wmi_ffi::read_only_attr(
    CURRENT_LEVEL_ATTR_NAME.as_ptr(),
    current_level_show,
));
static CURRENT_LEVEL_RAW_ATTR: DeviceAttr = DeviceAttr(wmi_ffi::read_only_attr(
    CURRENT_LEVEL_RAW_ATTR_NAME.as_ptr(),
    current_level_raw_show,
));

module! {
    type: CorsairWmi,
    name: "corsair_wmi",
    authors: ["Local driver project"],
    description: "Read-only CORSAIR AI Workstation performance-level WMI Rust driver",
    license: "GPL",
    alias: ["wmi:8FAFC061-22DA-46E2-91DB-1FE3D7E5FF3C", "wmi:99D89064-8D50-42BB-BEA9-155B2E5D0FCD"],
}

struct CorsairWmi;

impl kernel::Module for CorsairWmi {
    fn init(_module: &'static ThisModule) -> Result<Self> {
        unsafe {
            kernel::bindings::mutex_init_generic(core::ptr::addr_of_mut!(METHOD_DEV_LOCK).cast());
        }

        // The Linux WMI subsystem still needs raw C registration. The embedded
        // `device_driver` is intentionally zeroed except for its name because
        // the WMI core owns bus binding and callback dispatch from here.
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
            // Publish the static registration object before handing its address
            // to the WMI core. It is unregistered in Drop before module unload.
            driver_ptr.write(driver);
            to_result(wmi_ffi::__wmi_driver_register(
                driver_ptr,
                core::ptr::addr_of_mut!(kernel::bindings::__this_module),
            ))?;
        }

        pr_info!("registered Rust WMI driver\n");
        Ok(Self)
    }
}

impl Drop for CorsairWmi {
    fn drop(&mut self) {
        let driver_ptr = core::ptr::addr_of_mut!(WMI_DRIVER).cast::<wmi_ffi::WmiDriver>();

        unsafe {
            wmi_ffi::wmi_driver_unregister(driver_ptr);
        }

        pr_info!("unregistered Rust WMI driver\n");
    }
}

unsafe extern "C" fn corsair_wmi_probe(
    wdev: *mut wmi_ffi::WmiDevice,
    context: *const c_void,
) -> c_int {
    pr_info!("probe callback\n");

    if wdev.is_null() {
        return -(kernel::bindings::ENODEV as c_int);
    }

    if context == wmi_ffi::METHOD_CONTEXT {
        // Only the method device receives sysfs files and the initial read-only
        // AA method query. The event device binds solely for notify_new().
        let ret = attach_method_device(wdev);
        if ret != 0 {
            return ret;
        }

        if let Err(ret) = query_current_level(wdev) {
            pr_info!("initial level query failed ret={}\n", ret);
        }
    }

    0
}

unsafe extern "C" fn corsair_wmi_remove(wdev: *mut wmi_ffi::WmiDevice) {
    if wdev.is_null() {
        return;
    }

    detach_method_device(wdev);

    pr_info!("remove callback\n");
}

unsafe extern "C" fn corsair_wmi_notify_new(
    _wdev: *mut wmi_ffi::WmiDevice,
    data: *const wmi_ffi::WmiBuffer,
) {
    let Some(payload) = selector_event_payload(data) else {
        return;
    };

    if !level::is_selector_event(&payload) {
        return;
    }

    let detail = payload[1];
    let level = level::Level::from_event_detail(detail);

    pr_info!(
        "selector event detail=0x{:02x} level_raw={}\n",
        detail,
        level.raw_value()
    );
    set_cached_level(level, "event");
}

fn attach_method_device(wdev: *mut wmi_ffi::WmiDevice) -> c_int {
    let dev = match wmi_device_dev(wdev) {
        Ok(dev) => dev,
        Err(ret) => return ret,
    };

    let _guard = MethodDevGuard::lock();
    if unsafe { !METHOD_DEV.is_null() } {
        pr_info!("method device already bound; rejecting duplicate\n");
        return -(kernel::bindings::EBUSY as c_int);
    }

    let ret = create_level_attrs(dev);
    if ret != 0 {
        return ret;
    }

    let referenced_dev = unsafe { kernel::bindings::get_device(dev) };
    if referenced_dev.is_null() {
        remove_level_attrs(dev);
        return -(kernel::bindings::ENODEV as c_int);
    }

    unsafe {
        METHOD_DEV = referenced_dev;
    }

    0
}

fn detach_method_device(wdev: *mut wmi_ffi::WmiDevice) {
    let Ok(dev) = wmi_device_dev(wdev) else {
        return;
    };

    let mut owned_dev = ptr::null_mut();
    {
        let _guard = MethodDevGuard::lock();
        if unsafe { METHOD_DEV == dev } {
            unsafe {
                owned_dev = METHOD_DEV;
                METHOD_DEV = ptr::null_mut();
            }
        }
    }

    if !owned_dev.is_null() {
        // METHOD_DEV is already cleared, so new notifications will not race the
        // sysfs teardown below.
        remove_level_attrs(dev);
        unsafe {
            kernel::bindings::put_device(owned_dev);
        }
    }
}

fn create_level_attrs(dev: *mut kernel::bindings::device) -> c_int {
    if dev.is_null() {
        return -(kernel::bindings::ENODEV as c_int);
    }

    // Attach files directly to the method WMI device, matching the public ABI
    // documented in README.md.
    let ret = unsafe { wmi_ffi::device_create_file(dev, core::ptr::addr_of!(CURRENT_LEVEL_ATTR.0)) };
    if ret != 0 {
        return ret;
    }

    let ret =
        unsafe { wmi_ffi::device_create_file(dev, core::ptr::addr_of!(CURRENT_LEVEL_RAW_ATTR.0)) };
    if ret != 0 {
        unsafe {
            wmi_ffi::device_remove_file(dev, core::ptr::addr_of!(CURRENT_LEVEL_ATTR.0));
        }
    }

    ret
}

fn remove_level_attrs(dev: *mut kernel::bindings::device) {
    if dev.is_null() {
        return;
    }

    unsafe {
        wmi_ffi::device_remove_file(dev, core::ptr::addr_of!(CURRENT_LEVEL_RAW_ATTR.0));
        wmi_ffi::device_remove_file(dev, core::ptr::addr_of!(CURRENT_LEVEL_ATTR.0));
    }
}

fn query_current_level(wdev: *mut wmi_ffi::WmiDevice) -> core::result::Result<(), c_int> {
    let level = evaluate_current_level_method(wdev)?;

    set_cached_level(level, "query");
    Ok(())
}

fn evaluate_current_level_method(wdev: *mut wmi_ffi::WmiDevice) -> core::result::Result<level::Level, c_int> {
    if wdev.is_null() {
        return Err(-(kernel::bindings::ENODEV as c_int));
    }

    // Method id 2 is the read-only current-level query. Method id 1 is not used
    // by this driver because firmware treats it as a state-changing path.
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
            METHOD_ID_CURRENT_LEVEL,
            core::ptr::addr_of!(input),
            core::ptr::addr_of_mut!(output),
        )
    };
    if status != 0 {
        return Err(-(kernel::bindings::EIO as c_int));
    }

    let output = AcpiAllocatedBuffer::new(output.pointer);
    let obj = output.as_object();
    if obj.is_null() {
        return Err(-(kernel::bindings::ENODATA as c_int));
    }

    let object_type = unsafe { (*obj).type_ };
    if object_type != kernel::bindings::ACPI_TYPE_INTEGER {
        return Err(-(kernel::bindings::ENODATA as c_int));
    }

    let value = unsafe { (*obj).integer.value };
    Ok(level::Level::from_query_value(value))
}

fn set_cached_level(level: level::Level, source: &'static str) {
    // The userspace-visible state is a single byte. Atomic storage is enough:
    // sysfs readers see either the previous complete level or the new one.
    let old = CURRENT_LEVEL.swap(level.raw_value(), Ordering::AcqRel);

    if old == level.raw_value() {
        pr_info!(
            "level={} raw={} source={} unchanged\n",
            level.as_str(),
            level.raw_value(),
            source
        );
    } else {
        pr_info!(
            "level={} raw={} source={}\n",
            level.as_str(),
            level.raw_value(),
            source
        );
    }

    if old != level.raw_value() {
        notify_level_attrs();
    }
}

fn notify_level_attrs() {
    let _guard = MethodDevGuard::lock();
    let dev = unsafe { METHOD_DEV };
    if dev.is_null() {
        return;
    }

    unsafe {
        // Wake pollers on both human-readable and numeric sysfs files.
        let kobj = core::ptr::addr_of_mut!((*dev).kobj);
        wmi_ffi::sysfs_notify(kobj, ptr::null(), CURRENT_LEVEL_ATTR_NAME.as_ptr());
        wmi_ffi::sysfs_notify(kobj, ptr::null(), CURRENT_LEVEL_RAW_ATTR_NAME.as_ptr());
    }
}

fn wmi_device_dev(
    wdev: *mut wmi_ffi::WmiDevice,
) -> core::result::Result<*mut kernel::bindings::device, c_int> {
    if wdev.is_null() {
        return Err(-(kernel::bindings::ENODEV as c_int));
    }

    Ok(unsafe { core::ptr::addr_of_mut!((*wdev).dev) })
}

fn selector_event_payload(data: *const wmi_ffi::WmiBuffer) -> Option<[u8; 3]> {
    if data.is_null() {
        return None;
    }

    let data = unsafe { &*data };
    if data.data.is_null() || data.length < 3 {
        return None;
    }

    // The driver only needs the first three event bytes. Avoid constructing a
    // slice over the firmware-provided full length.
    Some(unsafe {
        [
            ptr::read(data.data.cast::<u8>()),
            ptr::read(data.data.cast::<u8>().add(1)),
            ptr::read(data.data.cast::<u8>().add(2)),
        ]
    })
}

unsafe extern "C" fn current_level_show(
    _dev: *mut kernel::bindings::device,
    _attr: *mut kernel::bindings::device_attribute,
    buf: *mut u8,
) -> isize {
    if buf.is_null() {
        return -(kernel::bindings::EINVAL as isize);
    }

    let level = level::Level::from_raw(CURRENT_LEVEL.load(Ordering::Acquire));

    // sysfs_emit() is the kernel helper that bounds writes to PAGE_SIZE.
    unsafe { wmi_ffi::sysfs_emit(buf, b"%s\n\0".as_ptr(), level.name_cstr()) as isize }
}

unsafe extern "C" fn current_level_raw_show(
    _dev: *mut kernel::bindings::device,
    _attr: *mut kernel::bindings::device_attribute,
    buf: *mut u8,
) -> isize {
    if buf.is_null() {
        return -(kernel::bindings::EINVAL as isize);
    }

    let level = CURRENT_LEVEL.load(Ordering::Acquire);

    unsafe { wmi_ffi::sysfs_emit(buf, b"%u\n\0".as_ptr(), wmi_ffi::u32_arg(level)) as isize }
}

struct MethodDevGuard;

impl MethodDevGuard {
    fn lock() -> Self {
        unsafe {
            kernel::bindings::mutex_lock(core::ptr::addr_of_mut!(METHOD_DEV_LOCK).cast());
        }
        Self
    }
}

impl Drop for MethodDevGuard {
    fn drop(&mut self) {
        unsafe {
            kernel::bindings::mutex_unlock(core::ptr::addr_of_mut!(METHOD_DEV_LOCK).cast());
        }
    }
}

struct AcpiAllocatedBuffer {
    pointer: *mut c_void,
}

impl AcpiAllocatedBuffer {
    fn new(pointer: *mut c_void) -> Self {
        Self { pointer }
    }

    fn as_object(&self) -> *mut kernel::bindings::acpi_object {
        self.pointer.cast()
    }
}

impl Drop for AcpiAllocatedBuffer {
    fn drop(&mut self) {
        if !self.pointer.is_null() {
            unsafe {
                wmi_ffi::kfree(self.pointer);
            }
        }
    }
}
