# PowderBot firmware

ESP32, 4 MB flash. Wi-Fi: **PowderBot** / **powderbot**.
Dashboard: http://192.168.71.1/; logs: http://192.168.71.1/console.

```sh
source "$HOME/export-esp.sh"
cargo build
espflash flash --partition-table partitions.csv --flash-size 4mb target/xtensa-esp32-espidf/debug/firmware
```

Run from this directory. The root `flash.sh` also runs core tests.

- Scale: UART2, TX16/RX17, 9600 8N1; keep it set to GN.
- OLED: SSD1306, SDA21/SCL22, address 0x3C.
- Motor: STEP33/DIR32/ENABLE25 (active low); beeper: GPIO14.
- Profiles persist in LittleFS. Trickle defaults: 50% speed, 100 ms pulses,
  300 ms settling. Speed and pulse duration are editable per profile.
- Target and tolerance settings are volatile.
- A new load can start only with a fresh scale reading within ±tolerance of
  zero. Empty and tare the scale first; blocked starts do not enter the load log.
- Dashboard load log: latest 200 completed loads, kept until reboot. Includes
  powder, target, measured weight, tolerance, and accepted/rejected result.
  Download CSV, Print, or Reset log to start a new series (stop dispensing first).
  Tare/Reset clears the overthrow warning but does not erase the load log.
  Times are estimates from the browser clock and device uptime; no network clock
  is required. Overthrows play two descending error bursts and show a red warning.
- Optional features: `motor-test` runs at every boot; `storage-test` checks
  persistence; `storage-init` may erase storage after a mount failure.
  Use storage features separately from `motor-test`.
- Keep partition offsets unchanged to preserve profiles.
