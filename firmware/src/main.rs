mod api;
mod beeper;
mod dashboard;
mod display;
mod pins;
mod scale;
mod stepper;
mod storage;
mod web;
mod web_log;

use std::thread;
use std::time::Duration;

use esp_idf_svc::hal::gpio::AnyIOPin;
use esp_idf_svc::hal::i2c::{I2cConfig, I2cDriver};
use esp_idf_svc::hal::ledc::{config::TimerConfig, LedcDriver, LedcTimerDriver, Resolution};
use esp_idf_svc::hal::peripherals::Peripherals;
use esp_idf_svc::hal::uart::{config, UartDriver};
use esp_idf_svc::hal::units::Hertz;

#[cfg(all(
    feature = "motor-test",
    any(feature = "storage-init", feature = "storage-test")
))]
compile_error!("Use storage bring-up features separately from motor-test");

fn main() -> anyhow::Result<()> {
    esp_idf_svc::sys::link_patches();

    let logs = web_log::init()?;

    let peripherals = Peripherals::take()?;
    let pins = pins::BoardPins::new(peripherals.pins);

    let mut motor = stepper::Stepper::new(
        peripherals.ledc.timer1,
        peripherals.ledc.channel1,
        pins.stepper_step,
        pins.stepper_dir,
        pins.stepper_enable,
    )?;
    motor.stop()?;

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

    // Keep the web console alive. Start it before the optional motor test so
    // test messages are available even after USB serial has been disconnected.
    // Declare the mount owner first: reverse drop order stops HTTP handlers
    // before unmounting their filesystem if main exits with an error.
    let state = std::sync::Arc::new(std::sync::Mutex::new(
        powderbot_core::dashboard::Dashboard::default(),
    ));
    let storage;
    let mut web_console = web::WebConsole::new(peripherals.modem, logs)?;
    // Retain the mount for the lifetime of main. Failure leaves scale and logs usable.
    storage = match storage::Storage::mount() {
        Ok(storage) => {
            #[cfg(feature = "storage-test")]
            if let Err(error) = storage.probe() {
                log::error!("Storage probe failed: {error:#}");
            }
            Some(storage)
        }
        Err(error) => {
            log::error!("Storage unavailable: {error:#}");
            None
        }
    };
    web_console.register_profiles(storage.as_ref().map(|storage| storage.profiles.clone()))?;
    web_console.register_dashboard(state.clone())?;
    if let Some(display) = display.as_mut() {
        if let Err(error) = display.show_network(&web_console.address) {
            log::warn!("{error}");
        }
    }
    thread::sleep(Duration::from_secs(2));

    log::info!("Scale test: UART2, board pins from pins.rs, 9600 8N1; motor disabled");
    if let Some(beeper) = &beeper {
        if let Err(error) = beeper.play(beeper::Sound::Boot) {
            log::warn!("Could not queue boot chime: {error}");
        }
    }
    #[cfg(feature = "motor-test")]
    stepper::run_test(&mut motor)?;

    let mut scale_failed = false;
    loop {
        let screen_result = match scale::read_weight(&uart) {
            Ok(weight) => {
                if let Ok(mut state) = state.lock() {
                    state.record(Ok(weight), std::time::Instant::now());
                }
                scale_failed = false;
                log::info!("Scale weight: {weight:.3} GN");
                match display.as_mut() {
                    Some(display) => display.show_weight(weight),
                    None => Ok(()),
                }
            }
            Err(error) => {
                if let Ok(mut state) = state.lock() {
                    state.record(Err(error.to_string()), std::time::Instant::now());
                }
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
