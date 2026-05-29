# CORSAIR AI Workstation Performance Mode Linux Driver

This repository is a Linux driver that exposes the CORSAIR
AI Workstation front-panel Performance Mode Selector state.

![Button on Corsair AI Workstation](Overview.png)

The driver has read-only support:

- report the current mode at driver probe time
- report mode changes when the front-panel selector is pressed
- expose the current mode to userspace through sysfs `cat /sys/bus/wmi/devices/99D89064-8D50-42BB-BEA9-155B2E5D0FCD/current_mode`
- does not change firmware state from Linux

## Current Status

Validated on one CORSAIR AI Workstation system:

- The selector state is available through ACPI WMI.
- The current mode can be queried through WMI method GUID
  `99D89064-8D50-42BB-BEA9-155B2E5D0FCD`, object id `AA`, method id `2`.
- Selector change events arrive through WMI event GUID
  `8FAFC061-22DA-46E2-91DB-1FE3D7E5FF3C`, notify id `0xBC`.
- Event payloads are 8-byte buffers. The first three bytes act like:

```text
byte 0: event family, expected 0x01 for selector events
byte 1: mode/event detail
byte 2: event subtype/status, observed 0x81 for selector events
```

Mode mappings:

```text
Current-mode query result:
0 = Balanced
1 = Max
2 = Quiet
3 = Super

Selector event byte 1:
0x11 = Quiet
0x12 = Balanced
0x13 = Max
0x14 = Super
```

The workstation firmware also emits unrelated WMI events on the same event GUID.
One commonly observed payload is:

```text
01 0a 81 00 00 00 00 00
```

That event is not a selector mode event and should be ignored by a production
driver unless additional behavior is intentionally added for it.

## Supported Target

This project intentionally targets Ubuntu 26.04 with Linux 7.0+ kernels. Older
distributions and older kernel toolchains are out of scope for now.

The dev container is also based on Ubuntu 26.04 so its compiler, glibc, and
kernel tooling match the supported host family closely enough for out-of-tree
module builds.

## Repository Contents

```text
rust/corsair_wmi_kernel/         Rust WMI/sysfs kernel driver
rust/corsair_performance_mode_core/
                                  Rust no_std-friendly decode crate
rust_kernel_probe/               Minimal Rust kernel module smoke test
Cargo.toml                        Rust crate manifest
Makefile                         Convenience wrapper for the Rust module build
rust/corsair_wmi_kernel/Makefile Kbuild file for the out-of-tree Rust module
scripts/build.sh                 Builds the Rust kernel module
scripts/sign_for_secure_boot.sh  Generates a local MOK cert and signs the module
scripts/check_kernel_rust.sh      Checks/builds the Rust kernel smoke module
LICENSE                          Repository license
```

The local Secure Boot private key is intentionally ignored. Each user must
generate and enroll their own key.

## Dev Container

The dev container installs Rust tooling plus the kernel tools needed for the
driver module. It also bind-mounts the host's `/lib/modules` and `/usr/src`
read-only so Kbuild can find matching kernel headers.

Kernel Rust builds use Ubuntu's packaged Rust compiler to match the
`linux-lib-rust-*` kernel libraries. The dev container sets:

```text
RUST_LIB_SRC=/opt/rustc-1.93.1/library
```

That path is copied from Ubuntu's `rust-src` package during image build because
the runtime `/usr/src` bind mount would otherwise hide the image's copy.

After changing `.devcontainer/Dockerfile` or `.devcontainer/devcontainer.json`,
rebuild the container before running the kernel module build:

```sh
./scripts/build.sh
```

## Running The Driver

Build:

```sh
./scripts/build.sh
```

Loading a kernel module changes the host kernel, so the `insmod`, `rmmod`, and
`dmesg` commands below must run on the host or in a container started with the
needed kernel-module privileges. A normal VS Code dev container can build and
sign the module, but may not be allowed to load it.

If Secure Boot is enabled, sign and enroll a local Machine Owner Key:

```sh
./scripts/sign_for_secure_boot.sh
sudo mokutil --import mok/corsair_wmi.der
```

If this repository already has an older ignored `mok/corsair_wmi_probe.der`
certificate from early local testing, the signing script will reuse it so an
already-enrolled MOK continues to work.

Reboot and enroll the key in the blue MOK manager screen if needed. To watch
the probe logs and then load:

```sh
sudo dmesg -w
sudo insmod corsair_wmi.ko
```

Press the front-panel selector. The driver should log decoded mode events.
It also exposes read-only sysfs attributes on the method WMI device:

```text
/sys/bus/wmi/devices/99D89064-8D50-42BB-BEA9-155B2E5D0FCD/current_mode
/sys/bus/wmi/devices/99D89064-8D50-42BB-BEA9-155B2E5D0FCD/current_mode_raw
```

Unload:

```sh
sudo rmmod corsair_wmi
```

## Rust Core

The Rust crate contains the stable decode contract in a form that can be tested
without loading a kernel module:

```sh
cargo test
cargo clippy --all-targets -- -D warnings
```

It is deliberately small and `no_std`-friendly:

- `Mode::from_query_value()`
- `Mode::from_event_detail()`
- `is_selector_event()`
- `decode_selector_event()`

The kernel driver has a local copy of the same tiny decode contract because
out-of-tree kernel Rust modules are built by Kbuild rather than Cargo.

## Rust Kernel Probe

The target kernel has Rust enabled, and out-of-tree Rust module builds need the
matching Ubuntu Rust compiler plus prebuilt Rust kernel libraries. Check the
host/container setup and build the smoke module with:

```sh
./scripts/check_kernel_rust.sh
```

If the script reports missing `libcore.rmeta`, `libkernel.rmeta`, or
`libpin_init.rmeta`, install the matching package on the host:

```sh
sudo apt install linux-lib-rust-$(uname -r)
```

Then rebuild/reopen the dev container so the `/usr/src` bind mount exposes that
package. The dev container uses Ubuntu's packaged `rustc` rather than rustup so
the compiler matches those kernel libraries. Once present, the script builds the
minimal smoke module in `rust_kernel_probe/`.

## Clean-Room Driver Implementation Guide

A production driver should bind to the two WMI GUIDs:

```text
8FAFC061-22DA-46E2-91DB-1FE3D7E5FF3C   event source, notify id 0xBC
99D89064-8D50-42BB-BEA9-155B2E5D0FCD   method device, object id AA
```

Recommended behavior:

1. Bind to both WMI devices.
2. When the method device probes, call method id `2` on instance `0`.
3. Decode integer return values as the current mode.
4. Cache the current mode in driver state.
5. When the event device receives a notification, accept only selector payloads:

```text
length >= 3
payload[0] == 0x01
payload[2] == 0x81
payload[1] in { 0x11, 0x12, 0x13, 0x14 }
```

6. Decode `payload[1]` as the new mode and update the cached mode.
7. Notify userspace when the cached mode changes.

Implemented sysfs interface:

```text
/sys/bus/wmi/devices/<method-guid>/current_mode
```

The file should be read-only and return one lowercase mode name:

```text
quiet
balanced
max
super
unknown
```

An optional numeric file is also useful for scripts:

```text
/sys/bus/wmi/devices/<method-guid>/current_mode_raw
```

Suggested raw values:

```text
0 = balanced
1 = max
2 = quiet
3 = super
255 = unknown
```

Mode-change notifications are emitted with `sysfs_notify()` on `current_mode`
and `current_mode_raw`. A userspace daemon can then poll the sysfs file or use
inotify-like mechanisms depending on the desired integration.

Do not call method id `1` for read-only support. Method id `1` is treated as the
mode setter by the firmware interface.

## Approaches Evaluated

These paths were useful to understand the platform but are not recommended as
the final implementation:

- ACPI GPE counters: selector presses often incremented a GPE counter, but the
  counter also changed for unrelated reasons and did not carry the mode value.
- Generic uevent monitoring from userspace: useful for seeing that WMI devices
  exist, but it did not expose the event payload.
- WMI BMOF/data block dumps: confirmed GUID/object metadata, but did not provide
  a reliable current-mode signal.
- Platform profile sysfs: the system exposed platform-profile-related kernel
  pieces, but not a usable profile state for this selector.

The WMI method/event path is the working approach.

## Rust Driver Notes

The supported target is Ubuntu 26.04 with Linux 7.0+ kernels, so the project
leans on the modern Rust kernel toolchain instead of carrying compatibility for
older distributions. Ubuntu's Rust kernel package does not expose a safe WMI
driver abstraction yet, so the Rust driver uses a small local FFI module for
`struct wmi_driver`, `wmidev_evaluate_method()`, WMI notifications, and sysfs
attributes.

Build the Rust driver with:

```sh
./scripts/build.sh
./scripts/sign_for_secure_boot.sh
```

## Licensing Notes

The repository is GPL-2.0-only. The WMI functions used by the driver are
exported by the kernel as GPL-only symbols, so the loadable kernel module also
declares a GPL-compatible module license string:

```text
SPDX-License-Identifier: GPL-2.0
MODULE_LICENSE("GPL")
```

## Known Open Questions

- Confirm the mode mapping against firmware setup UI on additional systems.
- Decide whether `Super` should be exposed as `super`, `max_plus`, or hidden as
  an unsupported value if the product documentation only names three modes.
- Decide whether the sysfs node should live on the method WMI device, the event
  WMI device, or a small platform device created by the driver.
- Decide final upstream strategy: out-of-tree module, DKMS package, or eventual
  kernel submission.
