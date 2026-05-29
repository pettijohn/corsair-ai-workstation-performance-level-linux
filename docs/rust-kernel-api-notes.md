# Rust Kernel API Notes

Target environment:

- Ubuntu 26.04 host
- Linux `7.0.0-15-generic` or newer 7.0+ Ubuntu kernel
- Matching `linux-lib-rust-$(uname -r)` package installed on the host
- `RUST_LIB_SRC` pointing at the Rust source library used by the kernel build

## What the Kernel Rust Tree Provides

The Ubuntu 7.0 kernel Rust package provides enough support to build an
out-of-tree Rust module:

- `kernel::Module` for module init/drop lifecycle
- `module!` metadata macro
- logging macros such as `pr_info!`
- `kernel::prelude::*`
- raw generated bindings through `kernel::bindings`
- generated ACPI and driver-model types such as:
  - `bindings::acpi_buffer`
  - `bindings::acpi_object`
  - `bindings::device`
  - `bindings::device_driver`
  - `bindings::wmi_device_id`

The real driver build now serves as the toolchain proof: `scripts/check_kernel_rust.sh`
checks `rustavailable`, verifies the prebuilt Rust kernel libraries, and builds
`corsair_wmi.ko` against the host kernel headers.

## What Is Missing for This Driver

The WMI subsystem does not currently have a safe Rust abstraction in the Ubuntu
7.0 kernel package. The generated bindings also do not include every WMI type or
function needed by this driver.

Available from generated bindings:

- `struct wmi_device_id`
- `struct acpi_buffer`
- `union acpi_object`
- `struct device`
- `struct device_driver`

Missing from generated bindings:

- `struct wmi_device`
- `struct wmi_buffer`
- `struct wmi_driver`
- `__wmi_driver_register()`
- `wmi_driver_unregister()`
- `wmidev_evaluate_method()`
- `wmidev_instance_count()`
- `wmidev_block_query()`

Those missing pieces are declared in `include/linux/wmi.h`, and the required
runtime symbols are exported by the kernel. For the Rust port, we hand-write the
minimal ABI surface in `rust/corsair_wmi_kernel/wmi_ffi.rs`.

## WMI ABI Surface Needed

The all-Rust driver needs this subset of Linux WMI:

- register one `struct wmi_driver`
- match two WMI GUIDs in the `wmi_device_id` table
- receive `probe` and `remove` callbacks
- receive `notify_new` callbacks for selector events
- later, call `wmidev_evaluate_method(instance=0, method_id=2)` on the method
  WMI device

The ABI layouts used by the local FFI module are copied from the host kernel's
`include/linux/wmi.h`. This is intentionally narrow: if a future kernel changes
the WMI ABI, the small Rust FFI module is the single place to re-check.

## Symbol Licensing

The WMI method/query helpers are exported as GPL-only kernel symbols on this
target kernel. That means a loadable module which calls them must advertise a
GPL-compatible kernel module license. This is separate from the repository
license for user-space code or documentation, but the loadable driver itself
needs a GPL-compatible module license if it uses those symbols.

## Current Rust Port Boundary

The Rust module has moved past the initial bind/unbind skeleton. It now owns the
active driver path:

- WMI driver registration
- method id `2` current-mode query
- selector event filtering and decoding
- read-only `current_mode` and `current_mode_raw` sysfs attributes

The old C shim has been retired from active source; its implementation history
is available in git.
