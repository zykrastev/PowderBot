# PowderBot

Run core tests, build normal firmware, and upload it without a serial monitor:

```sh
./flash.sh
```

Optionally specify the serial port: `./flash.sh --port /dev/ttyUSB0`.
The script stops if any step fails. It enables neither motor-test nor storage
initialization features. Connect to PowderBot Wi-Fi after uploading and open
http://192.168.71.1/.
