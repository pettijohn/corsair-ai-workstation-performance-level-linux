# Future WMI Data: Fan RPM and Temperature Telemetry

This document captures a future design for extending the `corsair_wmi` driver
beyond the front-panel performance level selector. The goal is to expose useful
read-only telemetry, especially fan RPM and temperature data, while keeping the
driver conservative and avoiding write/control behavior.

The current driver intentionally exposes only:

```text
/sys/bus/wmi/devices/99D89064-8D50-42BB-BEA9-155B2E5D0FCD/current_level
/sys/bus/wmi/devices/99D89064-8D50-42BB-BEA9-155B2E5D0FCD/current_level_raw
```

Future telemetry should preserve that read-only posture. The firmware surface
also contains writable controls for performance level, fan duty, keyboard
backlight, and other state. Those should remain out of scope unless a separate,
explicit design is written for control operations.

## Source Observations

The relevant firmware objects are in the system ACPI DSDT:

```text
WMI method GUID: 99D89064-8D50-42BB-BEA9-155B2E5D0FCD
WMI method object id: AA
ACPI device UID: IP3POWERSWITCH
ACPI method: _SB.WMIB.WMAA
```

`WMAA` takes three arguments. In Linux WMI terms, the driver invokes object
`AA` with a method id, and the WMI/ACPI layer dispatches to `WMAA`.

The current level reader already uses method id `2`. ACPI inspection shows that
the same method object has additional subcommands:

```text
method id 1     set performance level through EC field FCMI
method id 2     read current performance level from EC field FCMO
method id 3     set fan duty through EC fields FAN1/FAN2
method id 4     read packed fan tach bytes from FN1H/FN1L/FN2H/FN2L
method id 0x09  set keyboard backlight fields
method id 0x0A  read packed keyboard backlight/color-ish value
method id 0x0B  read packed thermal bytes from CPUT/GPUT/CPAD/GPAD
method id 0x0C  set additional feature/state flags
method id 0x0D  read additional OSD/state information
```

Only method ids `4` and `0x0B` are candidates for this telemetry extension.

## Proposed Scope

Add initial read-only sysfs attributes for raw telemetry:

```text
fan1_raw
fan2_raw
fan_raw
temperature_cpu_raw
temperature_gpu_raw
temperature_aux_raw
temperature_raw
```

After units are validated, add unit-bearing names or, preferably, Linux
hardware-monitoring attributes under `hwmon`:

```text
fan1_input
fan2_input
temp1_input
temp2_input
```

The sysfs attributes on the WMI device are useful during bring-up because they
can preserve raw firmware values. The `hwmon` interface is better for normal
Linux integration once the units are known.

## Non-Goals

- Do not expose fan duty setters.
- Do not expose performance-level setters.
- Do not expose keyboard backlight control.
- Do not infer temperature units without host validation.
- Do not assume this firmware interface is present on non-CORSAIR systems or
  older firmware revisions.

## Fan RPM Query

ACPI branch:

```text
WMAA Arg1 == 0x04
```

Input behavior:

- `Arg2` is converted to an integer.
- If `Arg2 == 1`, the method reads four EC bytes.
- Otherwise it returns sentinel `0x7FFFFFFF`.

EC fields read:

```text
FN1H
FN1L
FN2H
FN2L
```

Observed packing logic:

```text
Local3 = FN1H
Local4 = FN1L
Local5 = FN2H
Local6 = FN2L
Local4 <<= 8
Local5 <<= 16
Local6 <<= 24
Local2 = Local4 + Local3 + Local5 + Local6
Return(ToBuffer(Local2))
```

Likely byte layout in little-endian form:

```text
byte 0: FN1H
byte 1: FN1L
byte 2: FN2H
byte 3: FN2L
```

Probable decoded fan values:

```text
fan1_raw = (byte1 << 8) | byte0
fan2_raw = (byte3 << 8) | byte2
```

The field names suggest fan tach or RPM, but the exact unit still needs host
validation. The first implementation should expose both the packed raw value
and the two split raw 16-bit values. It should not label them as RPM until the
numbers are compared against a known source.

Validation ideas:

- Compare raw values against BIOS/UEFI hardware monitor fan RPM if available.
- Compare against `sensors` output if any EC/hwmon driver already reports fans.
- Record values at idle and under load. Real RPM should vary plausibly and stay
  within normal fan-speed ranges.
- Watch whether either field is zero when a fan is stopped or absent.

## Temperature Query

ACPI branch:

```text
WMAA Arg1 == 0x0B
```

Input behavior:

- `Arg2` is converted to an integer.
- If `Arg2 == 1`, the method reads four EC bytes.
- Otherwise it returns sentinel `0x7FFFFFFF`.

EC fields read:

```text
CPUT
GPUT
CPAD
GPAD
```

Observed packing logic:

```text
Local3 = CPUT
Local4 = GPUT
Local5 = CPAD
Local6 = GPAD
Local4 <<= 8
Local5 <<= 16
Local6 <<= 24
Local2 = Local4 + Local3 + Local5 + Local6
Return(ToBuffer(Local2))
```

Likely byte layout in little-endian form:

```text
byte 0: CPUT
byte 1: GPUT
byte 2: CPAD
byte 3: GPAD
```

Possible interpretation:

```text
temperature_cpu_raw = byte0
temperature_gpu_raw = byte1
cpu_padding_or_aux_raw = byte2
gpu_padding_or_aux_raw = byte3
```

`CPUT` and `GPUT` look like CPU and GPU temperature bytes. `CPAD` and `GPAD`
may be padding, auxiliary values, adapter offsets, or another temperature-like
status. The names are suggestive but not enough to publish units.

Validation ideas:

- Compare `CPUT` against `k10temp`, `zenpower`, or another CPU thermal source.
- Compare `GPUT` against AMD GPU sensors if present.
- Collect samples at idle, under CPU load, and under GPU load.
- Confirm whether values appear to be degrees Celsius, degrees Celsius plus an
  offset, or a firmware-specific code.
- Observe whether `CPAD`/`GPAD` are stable padding or live telemetry.

## Method Invocation Design

The current driver already has the WMI FFI needed to call the method object.
The telemetry extension can reuse that path with two new read helpers:

```text
query_fan_data_raw()
  method id: 0x04
  input: integer 1
  expected output: buffer or integer carrying 32 bits

query_temperature_data_raw()
  method id: 0x0B
  input: integer 1
  expected output: buffer or integer carrying 32 bits
```

The implementation should accept both ACPI integer and ACPI buffer outputs if
the kernel WMI layer can return either form for `ToBuffer(Local2)`. The current
level query returns an integer, while these branches explicitly return
`ToBuffer(Local2)`, so buffer handling is expected.

Recommended decode contract:

```rust
struct PackedFanData {
    raw: u32,
    fan1_raw: u16,
    fan2_raw: u16,
}

struct PackedTemperatureData {
    raw: u32,
    cpu_raw: u8,
    gpu_raw: u8,
    cpu_aux_raw: u8,
    gpu_aux_raw: u8,
}
```

The no-std core crate can own these decode helpers. The kernel module can keep a
small local copy, as it currently does for level decoding, until the build model
allows direct reuse.

## Sysfs ABI Sketch

Initial bring-up attributes on the method WMI device:

```text
/sys/bus/wmi/devices/99D89064-8D50-42BB-BEA9-155B2E5D0FCD/fan1_raw
/sys/bus/wmi/devices/99D89064-8D50-42BB-BEA9-155B2E5D0FCD/fan2_raw
/sys/bus/wmi/devices/99D89064-8D50-42BB-BEA9-155B2E5D0FCD/fan_raw
/sys/bus/wmi/devices/99D89064-8D50-42BB-BEA9-155B2E5D0FCD/temperature_cpu_raw
/sys/bus/wmi/devices/99D89064-8D50-42BB-BEA9-155B2E5D0FCD/temperature_gpu_raw
/sys/bus/wmi/devices/99D89064-8D50-42BB-BEA9-155B2E5D0FCD/temperature_aux_raw
/sys/bus/wmi/devices/99D89064-8D50-42BB-BEA9-155B2E5D0FCD/temperature_raw
```

Once units are validated:

```text
fan1_rpm
fan2_rpm
temperature_cpu_celsius
temperature_gpu_celsius
```

If the values are conventional RPM and Celsius, prefer `hwmon` for the stable
public interface:

```text
/sys/class/hwmon/hwmonX/name
/sys/class/hwmon/hwmonX/fan1_input
/sys/class/hwmon/hwmonX/fan2_input
/sys/class/hwmon/hwmonX/temp1_input
/sys/class/hwmon/hwmonX/temp1_label
/sys/class/hwmon/hwmonX/temp2_input
/sys/class/hwmon/hwmonX/temp2_label
```

For `hwmon`, temperatures must be reported in millidegrees Celsius. Do not add
`temp*_input` until the conversion is known.

## Caching and Polling

The current level cache updates from both query and event notifications. Fan
and temperature telemetry should be query-on-read at first:

- A sysfs read invokes the WMI method.
- The driver decodes the returned packed value.
- The driver prints a single value into the sysfs buffer.

If repeated reads are expensive or slow, add a small cache later:

```text
cache duration: 1000 ms
fan cache: PackedFanData + timestamp + validity flag
temperature cache: PackedTemperatureData + timestamp + validity flag
```

Reasons to start with query-on-read:

- It avoids inventing polling policy.
- It keeps the driver simple while units are being validated.
- It lets normal tools decide how often they want to read.

Reasons to add caching later:

- `sensors` and desktop status UIs may read repeatedly.
- WMI/ACPI method evaluation can be slower than reading normal memory-backed
  sysfs files.
- Firmware EC access may have locking or latency implications.

## Error Handling

The read helpers should fail closed:

- If the WMI call fails, return the kernel error from the sysfs read.
- If the output type is not integer or buffer, return `-EIO`.
- If the buffer is shorter than 4 bytes, return `-EIO`.
- If the returned raw value is `0x7FFFFFFF`, return `-ENODATA` or print
  `unknown`.
- If values are clearly out of expected range during bring-up, log once and
  expose the raw value rather than guessing.

Avoid noisy logs on every sysfs read. Use rate-limited logs for unexpected
firmware responses.

## Safety and Control Boundaries

The same `WMAA` method contains writable subcommands. The driver should make the
read-only boundary obvious in code:

```text
const METHOD_ID_CURRENT_LEVEL: u32 = 0x02;
const METHOD_ID_READ_FAN_DATA: u32 = 0x04;
const METHOD_ID_READ_TEMPERATURE_DATA: u32 = 0x0B;
```

Do not add constants for writable method ids in the production driver unless
they are needed for documentation comments or tests. If they are documented,
name them clearly as disabled/unsupported.

The telemetry read path should always pass input integer `1`. It should never
pass user-provided input into the WMI method.

## Testing Plan

Unit tests in the core crate:

- Decode packed fan raw `0x12345678` into byte/word order.
- Decode packed temperature raw `0x12345678` into four bytes.
- Verify sentinel handling for `0x7FFFFFFF`.
- Verify short-buffer rejection.

Kernel smoke test:

1. Build and sign the module.
2. Load on the host.
3. Read current level attributes to confirm existing behavior still works.
4. Read `fan_raw` several times at idle.
5. Read `temperature_raw` several times at idle.
6. Record `dmesg` for unexpected WMI output types or failures.
7. Run a CPU load and confirm CPU-related raw temperature changes plausibly.
8. Run a GPU load and confirm GPU-related raw temperature changes plausibly.
9. Compare values with BIOS or known Linux sensor output.

Suggested host capture format:

```text
timestamp
current_level
fan_raw
fan1_raw
fan2_raw
temperature_raw
temperature_cpu_raw
temperature_gpu_raw
sensors output
notes: idle / cpu-load / gpu-load / max-level / quiet-level
```

## Open Questions

- Are `fan1_raw` and `fan2_raw` already RPM, or are they tach counts that need a
  conversion?
- Are `CPUT` and `GPUT` degrees Celsius?
- What exactly are `CPAD` and `GPAD`?
- Does the firmware return a buffer consistently for method ids `4` and `0x0B`
  on Linux?
- Are these values valid on all CORSAIR AI Workstation firmware revisions?
- Does querying these methods have any side effects, such as clearing an EC
  latch or causing extra WMI events?

## Recommended Implementation Order

1. Add packed-data decode helpers and unit tests.
2. Add a temporary debug-only kernel path that logs raw method id `4` and `0x0B`
   responses at probe time or through explicit read-only sysfs files.
3. Add raw sysfs attributes once the output type and packing are confirmed.
4. Collect host validation samples against BIOS and Linux sensor data.
5. Rename or supplement raw attributes with unit-bearing names only after unit
   validation.
6. Consider `hwmon` registration once the driver can report conventional RPM and
   millidegree-Celsius values.
