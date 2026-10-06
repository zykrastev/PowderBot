mod api;
mod beeper;
mod control;
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

    // Drop HTTP handlers before unmounting storage.
    let storage;
    let _worker;
    let mut web_console = web::WebConsole::new(peripherals.modem, logs)?;
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
    let profiles = storage.as_ref().map(|s| s.profiles.clone());
    let mut initial = powderbot_core::dashboard::Dashboard::default();
    if let Some(profiles) = &profiles {
        match profiles.lock().unwrap_or_else(|e| e.into_inner()).active() {
            Ok(profile) => {
                initial.select(profile).map_err(anyhow::Error::msg)?;
            }
            Err(error) => log::warn!("Active profile unavailable: {error}"),
        }
    }
    #[cfg(feature = "motor-test")]
    stepper::run_test(&mut motor)?;
    let (control, events, worker) = control::spawn(motor, uart, initial)?;
    _worker = worker;
    web_console.register_profiles(profiles.clone(), control.clone())?;
    web_console.register_dashboard(control.clone(), profiles)?;
    if let Some(display) = display.as_mut() {
        if let Err(error) = display.show_network(&web_console.address) {
            log::warn!("{error}");
        }
    }
    thread::sleep(Duration::from_secs(2));

    log::info!(
        "Controller ready; motor disabled. Trickle pulse duration and settling delay come from the profile"
    );
    let play = |sound| {
        if let Some(beeper) = &beeper {
            if let Err(error) = beeper.play(sound) {
                log::warn!("Could not queue sound: {error}");
            }
        }
    };
    play(beeper::Sound::Boot);
    let mut connected = false;
    let mut previous = powderbot_core::controller::State::Idle;
    let mut last_log = std::time::Instant::now();
    let mut connected_until = std::time::Instant::now();
    loop {
        use powderbot_core::controller::State;
        while let Ok(event) = events.try_recv() {
            match event {
                control::Event::ScaleError(error) => {
                    log::warn!("Scale: {error}");
                    play(beeper::Sound::Error);
                }
                control::Event::State(state) => {
                    log::info!("Controller: {}", state.name());
                    match state {
                        State::Finished => play(beeper::Sound::Finished),
                        State::Overthrown => play(beeper::Sound::Wrong),
                        State::Error => play(beeper::Sound::Error),
                        State::Coarse | State::Fine | State::Trickling
                            if matches!(
                                previous,
                                State::Idle
                                    | State::Settling
                                    | State::Finished
                                    | State::Overthrown
                                    | State::Error
                            ) =>
                        {
                            play(beeper::Sound::Dispensing)
                        }
                        _ => {}
                    }
                    previous = state;
                }
            }
        }
        let now = std::time::Instant::now();
        if let Ok(has_client) = web_console.has_client() {
            if has_client != connected {
                connected = has_client;
                log::info!(
                    "WiFi client {}",
                    if connected {
                        "connected"
                    } else {
                        "disconnected"
                    }
                );
                if connected {
                    play(beeper::Sound::Connected);
                    connected_until = now + Duration::from_millis(500);
                    if let Some(display) = display.as_mut() {
                        if let Err(e) = display.show_connected() {
                            log::warn!("{e}");
                        }
                    }
                } else {
                    play(beeper::Sound::Error);
                }
            }
        }
        match control.snapshot() {
            Ok(state) => {
                if now.saturating_duration_since(last_log) >= Duration::from_secs(1) {
                    if let Some((weight, _)) = state.reading(now) {
                        log::info!(
                            "Scale weight: {weight:.2} GN; {}",
                            state.controller.state().name()
                        );
                    }
                    last_log = now;
                }
                if now >= connected_until {
                    if let Some(display) = display.as_mut() {
                        let result = if connected || state.controller.state().active() {
                            display.show_status(&state, now)
                        } else {
                            display.show_network(&web_console.address)
                        };
                        if let Err(error) = result {
                            log::warn!("{error}");
                        }
                    }
                }
            }
            Err((_, error)) => log::error!("{error}"),
        }
        thread::sleep(Duration::from_millis(100));
    }
}
