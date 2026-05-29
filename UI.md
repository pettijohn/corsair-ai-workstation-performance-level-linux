Here is a design-document version you can use as a project brief.

# Corsair AI Workstation GNOME Performance Mode Indicator

## Requirements

Build a small native desktop indicator for Ubuntu 26.04 with GNOME that displays the current Corsair AI Workstation performance mode exposed by the installed kernel module:

```text
/sys/bus/wmi/devices/99D89064-8D50-42BB-BEA9-155B2E5D0FCD/current_mode
/sys/bus/wmi/devices/99D89064-8D50-42BB-BEA9-155B2E5D0FCD/current_mode_raw
```

The application should:

* Run as a small background desktop app.
* Show an icon in the GNOME top-right panel area.
* Update the icon when the current performance mode changes.
* Avoid periodic polling.
* Use the kernel module’s `sysfs_notify()` behavior to sleep until the sysfs value changes.
* Provide a click/flyout menu showing the current mode name.
* Be scoped to Ubuntu 26.04 with GNOME, not all Linux desktops.
* Prefer a native implementation over Electron, web UI, or heavyweight GUI frameworks.
* Start automatically with the user’s GNOME session.
* Allow the user to toggle GNOME session autostart from the menu.
* Remain small, maintainable, and easy to install locally.

Tooltip support is not required. GNOME/AppIndicator tooltip behavior is unreliable, so the current mode should be displayed in the flyout menu instead.

## Technical Considerations

### GNOME Panel Integration

GNOME does not use the old Windows-style “system tray” model directly. On Ubuntu GNOME, small panel indicators are typically exposed through the StatusNotifierItem / AppIndicator / KStatusNotifierItem ecosystem. Ubuntu’s GNOME AppIndicator extension is the compatibility layer that makes these indicators appear in the top-right panel.

The application should therefore target StatusNotifierItem/AppIndicator-style integration rather than an old X11 tray icon API.

### Tooltip Limitation

Tooltips are not a dependable feature in GNOME Shell indicators. Rather than depending on hover text, the app should show the current mode in its click menu:

```text
Mode: Balanced
──────────────
Quit
```

This is simpler and more consistent with GNOME behavior.

### Sysfs Notification Model

The kernel module exposes mode state through sysfs and emits `sysfs_notify()` when the value changes. Userspace should not reread the sysfs file on a timer.

Instead, the app should:

1. Open the `current_mode` sysfs file.
2. Read the initial value.
3. Block in `poll()`/equivalent on the file descriptor using `POLLPRI | POLLERR`.
4. When awakened, seek back to the start of the file.
5. Read the new mode.
6. Update the tray icon and menu.

Although the syscall is named `poll()`, this is not periodic polling. The process sleeps until the kernel wakes it.

### Mode Model

The app should parse textual modes from `current_mode`.

Expected values:

```text
quiet
balanced
max
super
unknown
```

The UI can show the exact textual mode in the menu, while mapping icons more coarsely if desired. If only three icons are desired, map `max` and `super` to the same high-performance icon.

Suggested mapping:

```text
quiet     → quiet icon
balanced  → balanced icon
max       → high-performance icon
super     → high-performance icon
unknown   → unknown/fallback icon
```

## Recommended Rust Approach

The recommended implementation is a single Rust binary using:

* `ksni` for StatusNotifierItem integration.
* `nix` or a similar crate for `poll()`.
* `crossbeam-channel` or standard channels for passing mode updates from the watcher thread to the UI/tray state.
* Named SVG icons installed into the user icon theme.
* A GNOME autostart `.desktop` file.

Recommended architecture:

```text
Rust binary
  ├─ main tray/status-notifier service
  │    ├─ owns current mode state
  │    ├─ exposes panel icon
  │    └─ exposes click menu
  │
  └─ watcher thread
       ├─ opens /sys/.../current_mode
       ├─ reads initial mode
       ├─ blocks in poll()
       ├─ wakes on sysfs_notify()
       └─ sends mode changes to main app
```

### Why Rust

Rust is a good fit because the application is small, long-running, and system-adjacent. It can provide a single native binary, strong type safety, clean enum modeling of modes, and direct access to Linux file descriptor APIs without needing a large runtime.

Rust also gives two viable implementation styles:

1. A cleaner StatusNotifierItem-based app using `ksni`.
2. A fallback higher-level tray implementation using `tray-icon`.

For this project, `ksni` is the preferred first attempt because it maps directly to StatusNotifierItem, which is the protocol family Ubuntu GNOME already supports through its AppIndicator extension.

## Proposed Project Layout

```text
corsair-mode-indicator/
  Cargo.toml
  src/
    main.rs
    mode.rs
    watcher.rs
    indicator.rs
    autostart.rs
  icons/
    corsair-mode-quiet.svg
    corsair-mode-balanced.svg
    corsair-mode-max.svg
    corsair-mode-unknown.svg
  packaging/
    corsair-mode-indicator.desktop
    corsair-mode-indicator-autostart.desktop
```

### Core Types

Represent the mode as an enum:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Quiet,
    Balanced,
    Max,
    Super,
    Unknown,
}
```

The enum should provide:

* `parse(&str) -> Mode`
* `label() -> &'static str`
* `icon_name() -> &'static str`

Example labels:

```text
Quiet
Balanced
Max
Super
Unknown
```

Example icon names:

```text
corsair-mode-quiet-symbolic
corsair-mode-balanced-symbolic
corsair-mode-max-symbolic
corsair-mode-super-symbolic
corsair-mode-unknown-symbolic
```

## Sysfs Watcher Design

The watcher is responsible for all interaction with the kernel module’s sysfs file.

Pseudo-flow:

```text
open current_mode
read initial mode
send initial mode to UI
loop:
  poll(fd, POLLPRI | POLLERR, forever)
  seek fd to offset 0
  read current_mode
  parse mode
  if mode changed:
    send mode to UI
```

The watcher should be tolerant of transient errors but should report failures clearly when launched from a terminal. For example, if the sysfs file does not exist, the app should show `Unknown` or exit with a useful error depending on the desired behavior.

For early development, exiting with a clear error is preferable. For a polished desktop utility, showing an unknown/offline state may be nicer.

## Indicator Design

The indicator should expose:

* ID: `corsair-mode-indicator`
* Title: `Corsair Mode`
* Category: hardware/system service
* Status: active
* Icon: based on current mode
* Menu:

  * disabled item: `Corsair Performance`
  * disabled item: `Mode: Balanced`
  * checkmark item: `Start automatically`
  * separator
  * `Quit`

Suggested menu:

```text
Corsair Performance
Mode: Balanced
Start automatically ✓
--------------
Quit
```

The title and mode items should be disabled so they read as identity/status rather than clickable actions.
The `Start automatically` item should toggle the per-user GNOME autostart desktop entry.

If the kernel module later adds a writable interface for changing performance mode, the menu could evolve into:

```text
Current mode: Balanced
──────────────
Quiet
Balanced ✓
Max
Super
──────────────
Quit
```

But the initial app should be read-only.

## Icon Strategy

Use named icons installed into the user’s icon theme rather than embedded pixmaps.

Install icons to:

```text
~/.local/share/icons/hicolor/scalable/status/
```

Example files:

```text
corsair-mode-quiet-symbolic.svg
corsair-mode-balanced-symbolic.svg
corsair-mode-max-symbolic.svg
corsair-mode-super-symbolic.svg
corsair-mode-unknown-symbolic.svg
```

Then refresh the icon cache:

```bash
gtk-update-icon-cache ~/.local/share/icons/hicolor
```

Using icon names keeps the tray implementation simpler and fits the GNOME/Linux desktop model better than manually sending raw image payloads over D-Bus.

## Autostart

Install the release binary to:

```text
~/.local/bin/corsair-mode-indicator
```

Create a normal GNOME launcher entry so the app remains discoverable even when
autostart is disabled:

```text
~/.local/share/applications/corsair-mode-indicator.desktop
```

Create a separate GNOME autostart entry:

```text
~/.config/autostart/corsair-mode-indicator.desktop
```

Suggested desktop entry:

```ini
[Desktop Entry]
Type=Application
Name=Corsair Mode Indicator
Comment=Shows Corsair AI Workstation performance mode in the GNOME panel
Exec=/home/travis/.local/bin/corsair-mode-indicator
Terminal=false
Categories=Utility;
X-GNOME-Autostart-enabled=true
```

The `Start automatically` menu item should update only the autostart entry by
setting `X-GNOME-Autostart-enabled=true` or `false`. It should not use
`Hidden=true`, because the app should remain discoverable in GNOME's launcher
through the normal application desktop entry.

The app should also be runnable manually from a terminal during development:

```bash
cargo run
```

and later:

```bash
~/.local/bin/corsair-mode-indicator
```

## Development Plan

### Phase 1: Minimal Rust Prototype

Implement:

* Mode enum.
* Sysfs read.
* Sysfs watcher using `poll()`.
* Console logging on mode changes.

Success condition:

```text
Changing the hardware performance mode causes the Rust app to print the new mode without periodic polling.
```

### Phase 2: Add Indicator

Add `ksni` and expose:

* One panel icon.
* One disabled menu item showing current mode.
* One quit item.

Success condition:

```text
The icon appears in the GNOME top-right panel, and clicking it shows the current mode.
```

### Phase 3: Dynamic Icon Updates

Update the indicator icon and menu label whenever the watcher sends a new mode.

Success condition:

```text
Changing performance mode updates the visible icon and menu text.
```

### Phase 4: Install and Autostart

Add:

* Release build.
* User-local binary install.
* Icon install.
* GNOME autostart `.desktop` file.

Success condition:

```text
The indicator starts automatically after login and reflects the current mode.
```

## Recommended Dependencies

Likely Rust dependencies:

```toml
[dependencies]
anyhow = "1"
crossbeam-channel = "0.5"
nix = { version = "0.29", features = ["poll", "fs"] }
ksni = "0.3"
```

Exact versions may be adjusted during implementation based on current crate APIs.

System packages likely needed:

```bash
sudo apt install build-essential pkg-config
```

If using the fallback `tray-icon` approach, GTK development packages may also be needed:

```bash
sudo apt install libgtk-3-dev
```

## Options Considered but not Recommended

### GNOME Shell Extension in GJS

A GNOME Shell extension would provide the most native top-panel integration and could feel like part of the shell itself. However, GNOME Shell extension APIs change over time, and direct sysfs file descriptor watching from GJS is less pleasant than doing it in a normal native process.

This remains a good option if deeper GNOME integration is eventually desired, but it is more maintenance-heavy than a small Rust indicator app.

### Python AppIndicator App

A Python AppIndicator prototype would be the fastest way to prove the concept. It can use PyGObject, Ayatana AppIndicator, and GLib file descriptor watches to build a working tray indicator quickly.

It was not chosen as the preferred final design because the goal shifted toward a native compiled app. Python remains a good throwaway prototype option.

### Go AppIndicator / Systray App

A Go version using `getlantern/systray` would also be a strong option. It can produce a small single binary, use `unix.Poll`, embed icons, and expose a simple tray menu.

Rust was preferred because it has a cleaner path to StatusNotifierItem through crates like `ksni`, models the system-adjacent code nicely, and avoids some of the cross-platform abstraction baggage.

### Rust `tray-icon` Crate

The Rust `tray-icon` crate is a practical fallback. It has a higher-level tray API and is maintained in the Tauri ecosystem, which makes it attractive if `ksni` proves awkward.

It was not selected as the first choice because on Linux it uses GTK, while `ksni` maps more directly to the StatusNotifierItem protocol and better matches the tiny-indicator nature of this app.

### Direct D-Bus StatusNotifierItem Implementation

The purest implementation would manually implement StatusNotifierItem and DBusMenu interfaces using a D-Bus crate such as `zbus`. This would avoid higher-level tray libraries and give complete control over the protocol.

It is not recommended initially because DBusMenu is fiddly, and the project does not need custom protocol behavior. Existing libraries should handle that plumbing.

### `libayatana-appindicator` Bindings

Using Rust bindings to `libayatana-appindicator` would target Ubuntu’s AppIndicator stack directly. This is conceptually close to how Ubuntu GNOME exposes app indicators.

It was not chosen because it involves C library bindings and a GTK3-era AppIndicator stack. A Rust-native StatusNotifierItem crate is cleaner for a small new utility.

### Electron, Tauri Full App, or Web UI

A web-based or Electron-style app would be excessive for a one-icon indicator. It would add unnecessary runtime weight, memory usage, packaging complexity, and UI surface area.

This application should be a tiny native background utility, not a full desktop application.
