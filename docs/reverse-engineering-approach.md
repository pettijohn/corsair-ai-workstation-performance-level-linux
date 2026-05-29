# Reverse Engineering Approach: Windows OSD to Linux WMI Probe

This document summarizes the investigation that began with the CORSAIR Windows
Performance Level OSD `.exe` and ended with a working read-only C kernel module.
It intentionally stops before the later request to create a clean-room driver
design/repository.

The goal throughout this phase was narrow: determine whether Linux could read
the current front-panel Performance Level Selector state without changing it.

## Tools Used

Unpacking and local inspection:

- `innoextract`: unpacked the Windows installer.
- Python virtual environment: used for local prototype/reverse-engineering
  helper scripts.
- Python helper scripts created during the investigation, especially
  `tools/watch_power_selector.py`.
- Shell/file inspection tools: `rg`, `strings`, `find`, `cat`, and normal
  filesystem inspection of extracted files.

Linux hardware and firmware inspection:

- `/sys/bus/wmi/devices/...`: inspected Linux WMI device metadata.
- `/sys/firmware/acpi/interrupts/...`: watched ACPI GPE counters.
- `tools/dump_wmi_bmof.sh`: dumped WMI sysfs metadata and available BMOF blobs.
- `tools/dump_acpi_wmi.sh`: dumped/disassembled ACPI tables and searched for
  WMI references.
- ACPICA tooling via the ACPI dump/disassembly flow, including `iasl`-style
  AML-to-ASL disassembly.
- `rg` over dumped ACPI/WMI output.

Kernel probing:

- C WMI probe module, initially `tools/wmi_probe/corsair_wmi_probe.c`.
- Kbuild/`make` for out-of-tree kernel module builds.
- `dmesg -w` for live kernel logs.
- `modinfo`, `insmod`, and `rmmod` for module testing.

Secure Boot signing:

- `openssl`: generated local signing key/certificate.
- Kernel `scripts/sign-file`: signed the kernel module.
- `mokutil`: enrolled the Machine Owner Key certificate.

No decryption tool was ultimately needed. The useful signal came from unpacking,
metadata inspection, ACPI/WMI analysis, and live kernel probing rather than from
decrypting any payload.

## Phase 1: Unpack the Windows Installer

Question:

- What does the Windows OSD installer contain, and is there enough configuration
  or executable logic to identify the hardware interface?

Attempt:

- The original artifact was a Windows `.exe`.
- We initially treated it as something that needed unpacking rather than
  execution.
- Once `innoextract` was available, the installer was unpacked.

Artifacts observed:

- `extracted/app/config.ini`
- `extracted/app/readme.txt`
- `extracted/app/InstallService.bat`
- Windows OSD/service binaries and supporting app files

What worked:

- `innoextract` successfully exposed the installer payload.
- The extracted app confirmed that this was a Windows on-screen-display/service
  style utility for showing performance level changes.

What did not directly solve the problem:

- The extracted files did not provide a clean Linux-readable API.
- The user did not need a runnable reconstruction of the Windows application.
- The useful task became understanding the hardware/firmware interface, not
  rebuilding the Windows OSD.

What we learned:

- The Windows tool existed to display state changes, but Linux needed a direct
  read-only path.
- The documentation's three user-facing levels were `Quiet`, `Balanced`, and
  `Max`, while later firmware observations also exposed a fourth raw value that
  we called `Super`.

Next step:

- Stop focusing on runnable Windows source and look for platform firmware
  interfaces exposed to Linux.

## Phase 2: Build a Userspace Event Watcher

Question:

- Does pressing the front-panel selector produce any observable Linux event that
  can be used from userspace?

Attempt:

- Created a Python prototype, `tools/watch_power_selector.py`.
- Enumerated Linux WMI devices.
- Watched ACPI GPE counters.
- Added optional raw output and a temporary cycle-inference level.

Observed WMI devices:

```text
05901221-D566-11D1-B2F0-00A0C9062910 data   object_id=BA instance_count=1
05901221-D566-11D1-B2F0-00A0C9062910 data   object_id=BA instance_count=1
8FAFC061-22DA-46E2-91DB-1FE3D7E5FF3C event  notify_id=BC instance_count=1
99D89064-8D50-42BB-BEA9-155B2E5D0FCD method object_id=AA instance_count=1
```

Observed GPE behavior:

- Pressing the selector often changed `gpe0A` and aggregate GPE counters.
- Example deltas included `+2` and `+4`.
- There were more GPE changes than physical button presses.

What worked:

- The watcher identified a likely WMI event source:
  `8FAFC061-22DA-46E2-91DB-1FE3D7E5FF3C`.
- It also identified a likely query/control method device:
  `99D89064-8D50-42BB-BEA9-155B2E5D0FCD`.
- It showed that the front-panel selector does wake ACPI/WMI machinery.

What failed or was insufficient:

- Inferring level by cycling on GPE changes was not reliable.
- The system emitted GPE changes for reasons other than button presses.
- Cycle inference required knowing the initial level and tracking every press
  across reboots, which was no better than doing it manually.

What we learned:

- ACPI GPE counters were useful as a hint that something happened, but not as a
  source of truth.
- The WMI event and method devices were the more promising path.

Next step:

- Decode WMI metadata and ACPI tables to understand the event payload and method
  device.

## Phase 3: Dump WMI Metadata and BMOF/Data Blocks

Question:

- Does Linux expose WMI metadata or BMOF data that names the relevant objects,
  methods, or event fields?

Attempt:

- Created/ran `tools/dump_wmi_bmof.sh`.
- Dumped WMI sysfs metadata under a `decompiled/wmi-bmof` output directory.
- Searched the dumped output with `rg` for strings such as:
  `IP3`, `WMIEvent`, `EventDetail`, `AA`, `BA`, `BC`, `Level`, and `Power`.

Observed output:

```text
05901221-D566-11D1-B2F0-00A0C9062910 data   object_id=BA
8FAFC061-22DA-46E2-91DB-1FE3D7E5FF3C event  notify_id=BC
99D89064-8D50-42BB-BEA9-155B2E5D0FCD method object_id=AA
```

What worked:

- Confirmed the WMI GUID roles visible from Linux.
- Confirmed the key object/notify IDs:
  - `BA`: data object
  - `BC`: event notification
  - `AA`: method object

What failed or was insufficient:

- The BMOF/data dump did not reveal rich symbolic names for the level state.
- Searching the dumped output did not yield obvious strings such as `Level` or
  `Power` beyond the object IDs.

What we learned:

- Linux knew about a WMI event device and a WMI method device.
- The path forward required either ACPI AML inspection or a kernel WMI consumer
  to receive the payloads.

Next step:

- Dump/disassemble ACPI tables and search for WMI GUID/method behavior.

## Phase 4: Dump and Inspect ACPI/WMI Tables

Question:

- Does the firmware AML show what the `AA` method does and how the `BC` event
  payload is formed?

Attempt:

- Ran `tools/dump_acpi_wmi.sh`.
- Dumped ACPI tables to a `decompiled/acpi` output directory.
- Disassembled DSDT/SSDT tables with ACPICA/`iasl` tooling.
- Searched the disassembled output for WMI GUIDs, object IDs, and method/event
  references.

What worked:

- ACPI inspection supported the interpretation that:
  - `99D89064-8D50-42BB-BEA9-155B2E5D0FCD` was the method device.
  - `8FAFC061-22DA-46E2-91DB-1FE3D7E5FF3C` was the event device.
  - method id `2` on object `AA` was read-only enough to query.
- The C probe comments later recorded that method id `2` read firmware state
  associated with `EC0.FCMO`.

What failed or was insufficient:

- ACPI inspection alone did not give us a convenient userspace API.
- It still did not prove the exact runtime event payload values for each level.

What we learned:

- Method id `2` was the current-level read path to test.
- Method id `1` should be avoided for the read-only goal because it appeared to
  be a state-changing path.

Next step:

- Write a read-only kernel WMI probe that binds to the relevant GUIDs, logs WMI
  notify payloads, and optionally invokes only method id `2`.

## Phase 5: Build the Read-Only C WMI Probe

Question:

- Can a Linux WMI driver receive the event payload and query the current level
  directly?

Attempt:

- Wrote a C kernel module, initially `tools/wmi_probe/corsair_wmi_probe.c`.
- Bound to both WMI GUIDs:
  - event GUID `8FAFC061-22DA-46E2-91DB-1FE3D7E5FF3C`
  - method GUID `99D89064-8D50-42BB-BEA9-155B2E5D0FCD`
- Implemented a `notify_new` callback to log raw event buffers.
- Added module parameters:
  - `query_current=1`: invoke `AA` method id `2` at probe time.
  - `log_other_events=1`: log non-selector events too.
  - `query_blocks=1`: query WMI data blocks for diagnostics.
- Built with Kbuild as an out-of-tree module.

Secure Boot obstacle:

- Initial load failed with:

```text
Loading of unsigned module is rejected
```

Resolution:

- Generated a local signing key/certificate with `openssl`.
- Signed the module using the kernel `scripts/sign-file` helper.
- Enrolled the certificate with `mokutil`.
- Rebooted and enrolled the MOK in firmware.

Kernel warning:

- Loading the probe produced:

```text
loading out-of-tree module taints kernel
```

Meaning:

- The kernel marked itself tainted because an externally built module was
  loaded.
- This was expected for an out-of-tree development probe.
- It did not mean the module had modified the selector state.

What worked:

- The module bound to both WMI devices.
- With `query_current=1`, invoking `AA` method id `2` returned an ACPI integer.
- Runtime logs decoded current-level query values:

```text
0 = Balanced
1 = Max
2 = Quiet
3 = Super
```

- Button presses produced WMI event buffers, and the first three bytes carried
  the selector event detail:

```text
01 11 81 = Quiet
01 12 81 = Balanced
01 13 81 = Max
01 14 81 = Super
```

- The system also emitted unrelated event payloads, especially:

```text
01 0a 81 00 00 00 00 00
```

What failed or was insufficient:

- The old/deprecated WMI notify path and the new `notify_new` path could both
  surface similar data, creating duplicate-looking log lines during probing.
- The unrelated `01 0a 81 ...` event proved that simply watching any WMI event
  on the GUID was not enough; the driver needed a selector-event filter.

What we learned:

- The Linux read-only path was real and did not require the Windows OSD binary.
- The correct source of truth was:
  - method id `2` for initial/current level
  - filtered WMI event payloads for changes
- The selector filter needed to accept only:

```text
length >= 3
payload[0] == 0x01
payload[2] == 0x81
payload[1] in { 0x11, 0x12, 0x13, 0x14 }
```

Next step:

- Turn the C probe findings into a clean implementation plan.

This is the cutoff for this document.

## Final Findings at Cutoff

Known WMI devices:

```text
8FAFC061-22DA-46E2-91DB-1FE3D7E5FF3C   event source, notify id BC
99D89064-8D50-42BB-BEA9-155B2E5D0FCD   method device, object id AA
05901221-D566-11D1-B2F0-00A0C9062910   data object, object id BA
```

Known current-level query:

```text
WMI method object: AA
method id:         2
instance:          0
result type:       ACPI integer
```

Known query mapping:

```text
0 = Balanced
1 = Max
2 = Quiet
3 = Super
```

Known selector event mapping:

```text
01 11 81 = Quiet
01 12 81 = Balanced
01 13 81 = Max
01 14 81 = Super
```

Known non-selector event:

```text
01 0a 81 00 00 00 00 00
```

Read-only rule:

- Use method id `2`.
- Do not call method id `1`.

## Lessons Learned

- The Windows OSD binary was useful as a clue but not necessary for Linux
  support once the firmware WMI interface was identified.
- ACPI GPE counters were only a trigger hint, not a reliable state interface.
- WMI sysfs metadata revealed the important GUID/object/notify IDs but not the
  full level semantics.
- ACPI table inspection identified the likely read-only method path.
- A small C WMI probe was the decisive experiment because it could receive real
  event payloads and query the current level on actual hardware.
- Secure Boot signing was required for module loading on the target system, but
  it did not change the reverse-engineering conclusion.
- The working design at this cutoff was a read-only Linux WMI driver that
  queries current level through `AA` method id `2` and updates cached state from
  filtered `BC` selector events.
