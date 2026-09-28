# Scale and OLED bring-up

The firmware now polls the scale and logs raw replies and parsed grain values.
This milestone uses a blocking request/reply function with a 300 ms deadline,
then sleeps for one second to keep the output readable. It is not yet the
final controller loop.

Connections match the C++ firmware: UART2 TX GPIO16, RX GPIO17, 9600 baud,
8 data bits, no parity, 1 stop bit, no flow control. Each request is `1B 70`.
Replies must end with LF; an optional preceding CR is handled by the parser.
Only `GN` readings are accepted. No tare or unit-change command is sent.

Motor STEP GPIO33 is held low, ENABLE GPIO25 high (disabled, active low),
and DIR GPIO32 low. This describes application initialization, not pin levels
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
