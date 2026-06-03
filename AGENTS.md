# Agent Guide: Corsair AI Workstation Performance Driver

## Target Environment
- OS: Ubuntu 26.04
- Kernel: 7.0+ (with Rust enabled)
- Host Dependency: `sudo apt install linux-lib-rust-$(uname -r)`

## Architecture
- `rust/corsair_wmi_kernel`: Out-of-tree Rust kernel module. Built via Kbuild/Makefile, **not Cargo**.
- `rust/corsair_level_indicator`: GNOME status indicator (Cargo).
- `rust/corsair_performance_level_core`: Shared `no_std` decode logic (Cargo).
- **Note**: The kernel driver contains a local copy of the decode contract to avoid Cargo dependency in Kbuild.

## Developer Commands

### Kernel Driver
- **Build**: `./scripts/build.sh` (produces `corsair_wmi.ko`)
- **Build & Sign (Secure Boot)**: `./scripts/build.sh --sign`
- **Install & Load**: `./scripts/install.sh`
- **Install without build**: `./scripts/install.sh --no-build`
- **Uninstall**: `./scripts/uninstall.sh`
- **Toolchain Verify**: `./scripts/check_kernel_rust.sh`

### Optional UI
- **Build Only**: `./scripts/install-ui.sh --build-only`
- **Install**: `./scripts/install-ui.sh`
- **Uninstall**: `./scripts/uninstall-ui.sh`

### Rust Core
- **Test**: `cargo test`
- **Lint**: `cargo clippy --all-targets -- -D warnings`

## Critical Quirks
- **Compiler**: Use Ubuntu's packaged `rustc` to match kernel libraries. Dev container sets `RUST_LIB_SRC=/opt/rustc/library`.
- **Secure Boot**: If enabled, modules must be signed (`--sign`) and the MOK key imported via `sudo mokutil --import mok/corsair_wmi.der`.
- **Build Order**: The kernel driver must be installed and loaded before the UI can function.

## Verification
- **Current Level**: `cat /sys/bus/wmi/devices/99D89064-8D50-42BB-BEA9-155B2E5D0FCD/current_level`
- **Event Logs**: `sudo dmesg -w`
