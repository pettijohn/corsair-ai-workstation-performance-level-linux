# Host Test Checklist

Run these checks on the host machine after building or installing the module.

## One-Time Secure Boot

If Secure Boot is enabled, enroll the local certificate once:

```sh
./scripts/sign_for_secure_boot.sh
sudo mokutil --import mok/corsair_wmi.der
```

Reboot and enroll it in the firmware MOK screen. If you are reusing the older
ignored `mok/corsair_wmi_probe.der`, enroll that certificate instead.

## Manual Load Test

```sh
./scripts/build.sh
./scripts/sign_for_secure_boot.sh
sudo modprobe -r corsair_wmi 2>/dev/null || true
sudo insmod ./corsair_wmi.ko
```

Expected:

- `dmesg` shows the Rust WMI driver registering.
- `current_level` exists on the method WMI device.
- `current_level_raw` exists on the method WMI device.

```sh
cat /sys/bus/wmi/devices/99D89064-8D50-42BB-BEA9-155B2E5D0FCD/current_level
cat /sys/bus/wmi/devices/99D89064-8D50-42BB-BEA9-155B2E5D0FCD/current_level_raw
```

## Selector Event Test

Watch logs while pressing the front-panel selector:

```sh
sudo dmesg -w
```

Expected:

- Selector events log `detail=0x11`, `0x12`, `0x13`, or `0x14`.
- `current_level` changes to `quiet`, `balanced`, `max`, or `super`.
- Unrelated events such as `01 0a 81 ...` are ignored.

## Persistent Install Test

```sh
./scripts/install.sh
systemctl status systemd-modules-load.service --no-pager
cat /etc/modules-load.d/corsair_wmi.conf
modinfo corsair_wmi
```

If the module was already built and signed in the dev container, install only
from the host:

```sh
./scripts/install.sh --no-build
```

Reboot, then verify:

```sh
lsmod | grep '^corsair_wmi'
cat /sys/bus/wmi/devices/99D89064-8D50-42BB-BEA9-155B2E5D0FCD/current_level
```

The install script persists across reboots for the currently running kernel. Run
it again after a kernel upgrade until DKMS packaging exists.

## Uninstall Test

```sh
./scripts/uninstall.sh
test ! -e /etc/modules-load.d/corsair_wmi.conf
test ! -e /lib/modules/$(uname -r)/extra/corsair_wmi.ko
```
