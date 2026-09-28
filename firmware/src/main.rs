mod beeper;
mod display;
mod pins;
mod scale;

use std::thread;
use std::time::Duration;

use esp_idf_svc::hal::gpio::{AnyIOPin, PinDriver};
use esp_idf_svc::hal::i2c::{I2cConfig, I2cDriver};
use esp_idf_svc::hal::ledc::{config::TimerConfig, LedcDriver, LedcTimerDriver, Resolution};
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

    // Fixed soldered connections. Hold STEP low before configuring ENABLE.
    // Keep these drivers alive for the whole test. ENABLE is active low in
    // the original firmware, so HIGH disables the motor driver.
    let mut motor_step = PinDriver::output(pins.stepper_step)?;
    motor_step.set_low()?;
    let mut motor_enable = PinDriver::output(pins.stepper_enable)?;
    motor_enable.set_high()?;
    let mut motor_direction = PinDriver::output(pins.stepper_dir)?;
    motor_direction.set_low()?;

    let tone_config = TimerConfig::new()
        .frequency(Hertz(2_000))
        .resolution(Resolution::Bits8);
    let tone_timer = LedcTimerDriver::new(peripherals.ledc.timer0, &tone_config)?;
    let mut tone_pwm = LedcDriver::new(peripherals.ledc.channel0, &tone_timer, pins.beeper)?;
    tone_pwm.set_duty(0)?;
    let beeper = match beeper::Beeper::new(tone_pwm, tone_timer) {
        Ok(beeper) => Some(beeper),
        Err(error) => {
            log::error!("Could not start beeper task: {error}");
            None
        }
    };

    let i2c_config = I2cConfig::new().baudrate(Hertz(100_000));
    let oled = I2cDriver::new(peripherals.i2c0, pins.oled_sda, pins.oled_scl, &i2c_config)
        .map_err(|error| format!("I2C init: {error}"))
        .and_then(display::Display::new);

    let mut display = match oled {
        Ok(display) => {
            log::info!("OLED initialized: 128x64 at 0x3C");
            Some(display)
        }
        Err(error) => {
            log::error!("{error}; continuing with serial scale output");
            None
        }
    };

    // Keep the boot message visible during this bring-up milestone.
    thread::sleep(Duration::from_millis(500));

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
    if let Some(beeper) = &beeper {
        if let Err(error) = beeper.play(beeper::Sound::Boot) {
            log::warn!("Could not queue boot chime: {error}");
        }
    }
    let mut scale_failed = false;
    loop {
        let screen_result = match scale::read_weight(&uart) {
            Ok(weight) => {
                scale_failed = false;
                log::info!("Scale weight: {weight:.3} GN");
                match display.as_mut() {
                    Some(display) => display.show_weight(weight),
                    None => Ok(()),
                }
            }
            Err(error) => {
                // Sound once per failure episode, including a missing scale
                // at startup. A valid reading rearms the notification.
                if !scale_failed {
                    if let Some(beeper) = &beeper {
                        if let Err(error) = beeper.play(beeper::Sound::Error) {
                            log::warn!("Could not queue scale error tone: {error}");
                        }
                    }
                }
                scale_failed = true;
                log::warn!("Scale: {error}");
                let reason = match error {
                    scale::ScaleError::Timeout => "No reply (timeout)",
                    scale::ScaleError::InvalidReply(
                        powderbot_core::scale_protocol::ParseError::UnsupportedUnit,
                    ) => "Select GN on scale",
                    scale::ScaleError::Uart(_) | scale::ScaleError::IncompleteWrite => {
                        "UART communication"
                    }
                    _ => "Invalid scale reply",
                };
                match display.as_mut() {
                    Some(display) => display.show_scale_error(reason),
                    None => Ok(()),
                }
            }
        };
        if let Err(error) = screen_result {
            log::error!("{error}; OLED may show stale data");
        }

        // Slow bring-up polling keeps the serial output readable. The original
        // firmware's 100 ms cadence will return with the controller later.
        thread::sleep(Duration::from_secs(1));
    }
}
