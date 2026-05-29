I reviewed `README.md`. The hardware contract is clear enough to plan from: two WMI GUIDs, read-only current-mode query via method id `2`, selector events filtered by `01 {11..14} 81`, and a proposed read-only sysfs surface.

**Rust Prototype Plan**

1. **Start with a Rust decoding crate, not kernel Rust first**

   Build a small `no_std`-friendly Rust crate that owns the stable logic:

   - `Mode::{Quiet, Balanced, Max, Super, Unknown}`
   - `Mode::from_query_value(u8)`
   - `Mode::from_event_payload(&[u8])`
   - selector-event filter logic
   - unit tests for known payloads, especially ignoring `01 0a 81 ...`

   This gives us useful Rust code immediately without fighting kernel Rust on day one.

2. **Add a minimal C kernel shim around WMI**

   Keep the Linux WMI binding layer in C initially:

   - bind `8FAFC061-22DA-46E2-91DB-1FE3D7E5FF3C`
   - bind `99D89064-8D50-42BB-BEA9-155B2E5D0FCD`
   - call `wmidev_evaluate_method(..., method_id=2, ...)`
   - receive `notify_new`
   - expose sysfs attributes

   The shim can either duplicate the tiny decode logic at first, or call into Rust later once the build pipeline is proven.

3. **Expose the production sysfs interface**

   Implement:

   ```text
   /sys/bus/wmi/devices/<method-guid>/current_mode
   /sys/bus/wmi/devices/<method-guid>/current_mode_raw
   ```

   Read-only attributes:

   ```text
   current_mode: quiet | balanced | max | super | unknown
   current_mode_raw: 0 | 1 | 2 | 3 | 255
   ```

   On valid selector events, update cached state and call `sysfs_notify()`.

4. **Then attempt Rust-in-kernel integration** `[toolchain smoke test complete; Rust driver port next]`

   Ultimate goal: replace the C shim with a complete Rust kernel driver. No C
   should remain in the final driver path.

   Current probe result on Ubuntu 26.04 / Linux 7.0:

   - `CONFIG_RUST=y` and `CONFIG_RUST_IS_AVAILABLE=y` are present.
   - `RUST_LIB_SRC=/opt/rustc-1.93.1/library make -C /lib/modules/$(uname -r)/build rustavailable` passes after the dev container rebuild.
   - A minimal Rust kernel module smoke test exists in `rust_kernel_probe/`.
   - The smoke module requires the host package `linux-lib-rust-$(uname -r)`, which exposes:

     ```text
     /lib/modules/$(uname -r)/build/rust/libcore.rmeta
     /lib/modules/$(uname -r)/build/rust/libkernel.rmeta
     /lib/modules/$(uname -r)/build/rust/libpin_init.rmeta
     ```

   - With Ubuntu's packaged `rustc 1.93.1` and the matching `linux-lib-rust` package, `./scripts/check_kernel_rust.sh` builds `rust_kernel_probe/corsair_wmi_rust_probe.ko`.

   Host dependency:

   ```sh
   sudo apt install linux-lib-rust-$(uname -r)
   ```

   Then rebuild/reopen the dev container and verify:

   ```sh
   ./scripts/check_kernel_rust.sh
   ```

   Remaining steps toward the all-Rust driver:

   4.1. **Map the available Rust kernel APIs** `[complete]`

      Determine whether Ubuntu's Linux 7.0 Rust kernel crate exposes enough
      driver-model primitives for this driver:

      - `kernel::device`
      - `kernel::driver`
      - `kernel::sync`
      - sysfs/device-attribute helpers
      - raw bindings access through `kernel::bindings`
      - exported symbol access for `wmidev_evaluate_method()`
      - callback-compatible support for `struct wmi_driver`

      Deliverable: `docs/rust-kernel-api-notes.md` summarizes which pieces are
      safe wrappers, which require raw bindings, and which are missing.

   4.2. **Generate or hand-write minimal WMI FFI bindings** `[complete]`

      Build the thinnest Rust representation needed to bind a `struct
      wmi_driver`:

      - `struct wmi_device`
      - `struct wmi_buffer`
      - `struct wmi_device_id`
      - `struct wmi_driver`
      - `wmidev_evaluate_method()`
      - `wmidev_instance_count()` and `wmidev_block_query()` only if retaining
        debug functionality
      - ACPI object/buffer types needed to decode method id `2`

      Prefer generated bindings if the kernel build supports them cleanly. If
      generated bindings are too broad or unstable, hand-write a tiny local FFI
      module with exact ABI comments and compile checks.

      Deliverable: `rust/corsair_wmi_kernel/wmi_ffi.rs`.

   4.3. **Create a Rust WMI bind/unbind skeleton** `[complete]`

      Port only the binding lifecycle first:

      - module metadata name `corsair_wmi`
      - WMI id table for both GUIDs
      - `probe`
      - `remove`
      - `notify_new`
      - no sysfs yet
      - log enough to prove the Rust callbacks fire

      Deliverable: a signed Rust WMI skeleton that binds to both WMI GUIDs and
      can be loaded/unloaded on the host. This was later promoted to the final
      `corsair_wmi.ko` artifact in step 4.7.

   4.4. **Port read-only method query** `[complete]`

      Implement the method-device branch in Rust:

      - detect the method GUID
      - call `wmidev_evaluate_method(instance=0, method_id=2)`
      - decode an integer ACPI result through the Rust core mapping
      - cache mode in Rust state
      - preserve the rule: never call method id `1`

      Deliverable: Rust module calls AA method id `2`, decodes the integer
      result, caches it, and logs current mode on load.

   4.5. **Port selector event handling** `[complete]`

      Implement the event-device branch in Rust:

      - accept only `01 {11..14} 81` payloads
      - ignore unrelated OSD events by default
      - update cached mode from the Rust core crate

      Deliverable: Rust module filters selector events and logs/cache-updates
      mode changes on selector presses.

   4.6. **Port sysfs attributes** `[complete]`

      Expose the production ABI from Rust:

      ```text
      /sys/bus/wmi/devices/<method-guid>/current_mode
      /sys/bus/wmi/devices/<method-guid>/current_mode_raw
      ```

      Add `sysfs_notify()` equivalents on valid mode changes.

      Deliverable: Rust module exposes the same read-only `current_mode` and
      `current_mode_raw` attributes as the C driver. Host-side `cat` tests still
      need to be run on hardware.

   4.7. **Retire the C shim** `[complete]`

      Once Rust binding, query, event, and sysfs behavior match the C driver:

      - removed `src/corsair_wmi.c`
      - updated `Makefile`/scripts to build only the Rust module
      - keep the Rust core crate tests
      - kept the C implementation history only in git, not active source

      Deliverable: no C source is required to build/load the driver.

   4.8. **Hardening and packaging**

      After the all-Rust module works:

      - add a host test checklist for current mode, selector events, sysfs, and
        unload/reload
      - decide whether to keep the Rust smoke module or fold it into CI/docs
      - add DKMS or another repeatable install path
      - document Secure Boot signing with the `corsair_wmi` artifact name
      - consider whether `super` should remain public or be treated as unknown

5. **Devcontainer additions**

   For clean rebuilds, I’d add kernel/build tooling to `.devcontainer/Dockerfile`:

   ```text
   build-essential
   clang
   llvm
   lld
   libclang-dev
   bindgen
   pkg-config
   bc
   flex
   bison
   libelf-dev
   dwarves
   kmod
   openssl
   mokutil
   acpica-tools
   ```

   We also need access to matching host kernel headers. In a devcontainer, that usually means mounting `/lib/modules` and `/usr/src` from the host, or installing exact matching headers inside the container if available.

The initial Rust-core-plus-C-shim path has served its purpose. The active driver
path is now the all-Rust kernel module.
