# CORSAIR AI Workstation Performance Mode Linux Driver

This repository is a starting point for a Linux driver that exposes the CORSAIR
AI Workstation front-panel Performance Mode Selector state.

The goal is read-only support:

- report the current mode at driver probe time
- report mode changes when the front-panel selector is pressed
- expose the current mode to userspace through sysfs
- avoid changing firmware state from Linux

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

## Repository Contents

```text
src/corsair_wmi_probe.c          C WMI/sysfs kernel shim
rust/corsair_performance_mode_core/
                                  Rust no_std-friendly decode crate
Cargo.toml                        Rust crate manifest
Makefile                         Out-of-tree kernel module Makefile
scripts/build.sh                 Builds the prototype module in /tmp
scripts/sign_for_secure_boot.sh  Generates a local MOK cert and signs the module
LICENSE                          Repository license
```

The local Secure Boot private key is intentionally ignored. Each user must
generate and enroll their own key.

## Dev Container

The dev container installs Rust tooling plus the C/kernel tools needed for the
prototype module. It also bind-mounts the host's `/lib/modules` and `/usr/src`
read-only so Kbuild can find matching kernel headers.

After changing `.devcontainer/Dockerfile` or `.devcontainer/devcontainer.json`,
rebuild the container before running the kernel module build:

```sh
./scripts/build.sh
```

## Running The Prototype

Build:

```sh
./scripts/build.sh
```

If Secure Boot is enabled, sign and enroll a local Machine Owner Key:

```sh
./scripts/sign_for_secure_boot.sh
sudo mokutil --import mok/corsair_wmi_probe.der
```

Reboot, enroll the key in the blue MOK manager screen, then load:

```sh
sudo insmod corsair_wmi_probe.ko query_current=1
sudo dmesg -w
```

Press the front-panel selector. The prototype should log decoded mode events.
It also exposes read-only sysfs attributes on the method WMI device:

```text
/sys/bus/wmi/devices/99D89064-8D50-42BB-BEA9-155B2E5D0FCD/current_mode
/sys/bus/wmi/devices/99D89064-8D50-42BB-BEA9-155B2E5D0FCD/current_mode_raw
```

Unload:

```sh
sudo rmmod corsair_wmi_probe
```

Useful module parameters:

```text
query_current=1     Query method id 2 during probe and log the decoded mode
log_other_events=1  Log non-selector WMI events for investigation
query_blocks=1      Query WMI data blocks; currently not needed for mode support
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

The C kernel shim currently mirrors this tiny decode logic because the Linux WMI
boundary is still C. The intent is to keep the hardware contract tested in Rust
while the WMI/sysfs integration remains in the kernel-facing shim.

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

Rust is attractive for the final driver, but there are practical kernel issues:

- Rust-for-Linux support depends on the target kernel configuration. Many distro
  kernels still do not enable enough Rust support for comfortable out-of-tree
  modules.
- The Linux WMI subsystem may not have complete safe Rust abstractions in the
  target kernel version.
- If WMI bindings are missing, a Rust driver may need a small C shim or custom
  bindings around `struct wmi_driver`, `wmidev_evaluate_method()`, notify
  callbacks, and sysfs attributes.
- DKMS packaging for out-of-tree Rust modules is less routine than for C modules.

A practical path is:

1. Keep this C prototype as the hardware contract test.
2. Implement the production sysfs behavior in C first or as a minimal C shim.
3. Move non-WMI state handling and decoding into Rust if the target kernel has
   adequate Rust support.
4. Revisit a mostly-Rust implementation once the target kernel exposes stable
   WMI driver abstractions for Rust.

## Licensing Notes

The repository license is BSD-2-Clause, but Linux kernel module integration has
an extra constraint: the WMI functions used by this prototype are exported by
the kernel as GPL-only symbols. A module using those symbols must declare a
GPL-compatible kernel module license string or the kernel will reject access to
those exports.

For that reason the prototype source uses:

```text
SPDX-License-Identifier: BSD-2-Clause OR GPL-2.0-only
MODULE_LICENSE("Dual BSD/GPL")
```

That keeps the source available under BSD-2-Clause while also allowing the
kernel module to use the required GPL-only WMI exports when built for Linux.

## Known Open Questions

- Confirm the mode mapping against firmware setup UI on additional systems.
- Decide whether `Super` should be exposed as `super`, `max_plus`, or hidden as
  an unsupported value if the product documentation only names three modes.
- Decide whether the sysfs node should live on the method WMI device, the event
  WMI device, or a small platform device created by the driver.
- Decide final upstream strategy: out-of-tree module, DKMS package, or eventual
  kernel submission.
