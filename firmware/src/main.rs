mod pins;
mod scale;

use std::thread;
use std::time::Duration;

use esp_idf_svc::hal::gpio::{AnyIOPin, PinDriver};
use esp_idf_svc::hal::peripherals::Peripherals;
use esp_idf_svc::hal::uart::{config, UartDriver};
use esp_idf_svc::hal::units::Hertz;
use esp_idf_svc::sys::EspError;

fn main() -> Result<(), EspError> {
    // It is necessary to call this function once. Otherwise, some patches to the runtime
    // implemented by esp-idf-sys might not link properly. See https://github.com/esp-rs/esp-idf-template/issues/71
    esp_idf_svc::sys::link_patches();

    // Bind the log crate to the ESP Logging facilities
    esp_idf_svc::log::EspLogger::initialize_default();

    // Taking ownership prevents two drivers from claiming the same peripheral.
    let peripherals = Peripherals::take()?;
    let pins = pins::BoardPins::new(peripherals.pins);

    // Keep the remaining board pins for the OLED and beeper milestones.
    // Moving these handles does not initialize the hardware.
    let _reserved_pins = (pins.oled_sda, pins.oled_scl, pins.beeper);

    // Fixed soldered connections. Hold STEP low before configuring ENABLE.
    // Keep these drivers alive for the whole test. ENABLE is active low in
    // the original firmware, so HIGH disables the motor driver.
    let mut motor_step = PinDriver::output(pins.stepper_step)?;
    motor_step.set_low()?;
    let mut motor_enable = PinDriver::output(pins.stepper_enable)?;
    motor_enable.set_high()?;
    let mut motor_direction = PinDriver::output(pins.stepper_dir)?;
    motor_direction.set_low()?;

    let config = config::Config::default()
        .baudrate(Hertz(9_600))
        .data_bits(config::DataBits::DataBits8)
        .parity_none()
        .stop_bits(config::StopBits::STOP1)
        .flow_control(config::FlowControl::None);

    let uart = UartDriver::new(
        peripherals.uart2,
        pins.scale_tx,
        pins.scale_rx,
        Option::<AnyIOPin>::None,
        Option::<AnyIOPin>::None,
        &config,
    )?;

    log::info!("Scale test: UART2, board pins from pins.rs, 9600 8N1; motor disabled");
    loop {
        match scale::read_weight(&uart) {
            Ok(weight) => log::info!("Scale weight: {weight:.3} GN"),
            Err(error) => log::warn!("Scale: {error}"),
        }

        // Slow bring-up polling keeps the serial output readable. The original
        // firmware's 100 ms cadence will return with the controller later.
        thread::sleep(Duration::from_secs(1));
    }
}
