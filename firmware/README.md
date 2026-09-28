# Scale UART bring-up

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
during boot/reset. OLED and beeper are not initialized in this milestone.

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

If parsing fails, retain the raw reply from the serial log. The parser's
current format is based on the C++ example, not captured scale output.
An oversized reply or invalid text is rejected rather than truncated into a
potentially valid weight.
