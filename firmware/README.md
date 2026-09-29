# Scale and OLED bring-up

## Browser log viewer

The ESP32 creates Wi-Fi **PowderBot**, password **powderbot** (same as the
original C++ firmware). Connect a phone or computer, stay connected even if it
reports no internet, and open **http://192.168.71.1/**. The actual AP address
also appears briefly on the OLED at startup and is recorded in the log.
This address comes from the Rust network stack's default router configuration;
it differs from the original Arduino setup.

The page mirrors existing Rust `log::info!`, `warn!`, and `error!` messages and
refreshes once a second. It has no motor controls, shell, or debug commands.
Pause freezes only the browser view; firmware and logging continue running.
Messages are rendered as plain text, so raw scale output cannot inject HTML.

History keeps the latest 96 records, each capped at 384 UTF-8 bytes, in RAM.
It is available to newly connected clients, discards the oldest records when
full, and clears on reboot. ESP-IDF native C/ROM boot messages, panic output,
and direct `println!` calls are not intercepted; this mirrors Rust log macros.
Serial logging continues unchanged alongside the web view.

Build and flash **without opening the serial monitor**:

```sh
cd /data/Projects/PowderBot/core
cargo +stable fmt
cargo +stable test
cd ../firmware
source "$HOME/export-esp.sh"
cargo fmt
cargo build
espflash flash target/xtensa-esp32-espidf/debug/firmware
```

For the existing motor sweep, build with `cargo build --features motor-test`
and use the same `espflash flash` command. Do not add `--monitor`. The web server
starts before the unchanged automatic motor test. This image still moves the
motor once at EVERY boot/reset; the log page is read-only and cannot stop it.
After testing, build without the feature and flash again to restore the normal
stationary image.

Check: connect to the AP, see changing scale logs, pause/resume the page, reconnect
after switching Wi-Fi away, and confirm the motor sweep's percentage messages
appear when using the test image. Logger-history tests are in `core/tests/log_buffer.rs`.

The firmware now polls the scale and logs raw replies and parsed grain values.
This milestone uses a blocking request/reply function with a 300 ms deadline,
then sleeps for one second to keep the output readable. It is not yet the
final controller loop.

Connections match the C++ firmware: UART2 TX GPIO16, RX GPIO17, 9600 baud,
8 data bits, no parity, 1 stop bit, no flow control. Each request is `1B 70`.
Replies must end with LF; an optional preceding CR is handled by the parser.
Only `GN` readings are accepted. No tare or unit-change command is sent.

By default, motor STEP GPIO33 is held low, ENABLE GPIO25 high (disabled, active
low), and DIR GPIO32 high (forward). This describes application initialization, not pin levels
during boot/reset.

## Beeper

GPIO14 uses LEDC low-speed timer0/channel0 with 8-bit PWM. A dedicated task
owns the PWM drivers. The main loop sends requests through a bounded queue
using `try_send`, so neither note timing nor a full queue blocks scale polling.

After startup initialization, the boot chime plays 523, 659, and 784 Hz notes.
The first failed scale reading plays a 300 Hz tone for 500 ms. Further failures
remain silent until a valid reading rearms the notification. This also reports
a missing scale at startup and malformed replies. Reconnection itself is silent.

Test the startup chime, then switch off the scale: expect one error tone while
OLED and serial errors continue. Restore the scale, wait for a valid reading,
and switch it off again: expect one new error tone. Motor remains disabled.

## Motor driver and opt-in hardware test

`stepper.rs` provides a `Stepper` struct with `start`, `stop`, `set_speed_hz`,
`set_speed_percent`, `set_direction`, `is_running`, and `speed_hz` methods.
`new` initializes it stopped. Direction changes are accepted only while stopped.
Forward is DIR high, matching the C++ driver. Speed changes do not automatically
start a stopped motor; zero speed stops it, and starting at zero is rejected.
`stop` preserves the configured speed for a subsequent restart.

Hardware LEDC timer1/channel1 generates the STEP pulses independently of CPU
polling. Beeper uses timer0/channel0. Speed is 10..=2667 Hz with a roughly 50%
duty cycle. Percent commands accept 0..=100; tiny positive values have a 10 Hz
floor. Negative, non-finite, and out-of-range values are errors. Actual hardware
frequency is subject to timer quantization. Speed policy tests live in `core`.

There is no acceleration ramp or position/step counting in this milestone.
`stop` immediately raises ENABLE and then clears STEP duty; there is no queued
motion or deceleration. The driver releases holding torque, so physical coast
still depends on the mechanics. A successful API call is not motion feedback.
The 1 ms direction/enable setup waits are provisional until checked against
the installed motor driver's timing requirements.

For the deliberate hardware test, clear the dispenser so it runs unloaded:

```sh
cd /data/Projects/PowderBot/core
cargo +stable test
cd ../firmware
cargo fmt
cargo run --features motor-test
```

**This firmware image moves the motor once at EVERY boot/reset.** After a
3-second warning, it starts forward at 10% (~267 Hz), then sweeps to 100%
(2667 Hz). It holds every 10% level for two seconds and ramps between levels
in 1% increments spaced 20 ms apart. Motion lasts about 22 seconds, followed
by an immediate stop from 100%. Serial logs label each plateau so you can
compare noise and smoothness. The ramp belongs only to this test; ordinary
driver speed commands still apply directly. Verify smooth movement without
stalls and the final stop, and note any noisy speed bands.
Scale/OLED polling resumes after the test; this is a standalone motor check,
not weight-controlled dispensing.

After testing, restore the ordinary image, which never starts the motor:

```sh
cargo run
```

## OLED

`display.rs` owns an SSD1306 128x64 display, using I2C0 at 100 kHz on the
existing SDA GPIO21 / SCL GPIO22 pins, address `0x3C`. It shows PowderBot at
startup, then a weight in grains or a scale error. Each screen replaces the
previous framebuffer, so scale errors remove the previous weight.

If OLED initialization fails, serial scale polling continues. Later display
write errors are logged and another update is attempted on the next reading.
A failed I2C write can leave old pixels on the physical screen; consult the
serial log in that case. Initialization failure requires a restart to retry.

## Run yourself

```sh
cd /data/Projects/PowderBot/firmware
source "$HOME/export-esp.sh"
cargo fmt
cargo build
ESPFLASH_PORT=/dev/ttyUSB0 cargo run
```

Replace the port with the connected board's port.

## Check on hardware

1. Confirm the startup log and that the motor stays stationary.
2. Compare `Scale raw` and `Scale weight` with the scale's own display.
3. Change the load and confirm the next successful reply reflects it.
4. With the scale powered off, expect a timeout, not a zero-weight reading.
5. Restore the scale and confirm polling recovers without restarting the ESP32.
6. Confirm the OLED shows the startup message and agrees with the serial weight.
7. Turn off the scale: the OLED should replace the weight with a timeout message.
8. Restore the scale: the OLED should return to the current weight.

If parsing fails, retain the raw reply from the serial log. The parser's
current format is based on the C++ example, not captured scale output.
An oversized reply or invalid text is rejected rather than truncated into a
potentially valid weight.
