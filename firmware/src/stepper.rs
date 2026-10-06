
use std::thread;
use std::time::Duration;

use esp_idf_svc::hal::gpio::{Gpio25, Gpio32, Gpio33, Output, PinDriver};
use esp_idf_svc::hal::ledc::{
    config::TimerConfig, LedcDriver, LedcTimerDriver, LowSpeed, Resolution, CHANNEL1, TIMER1,
};
use esp_idf_svc::hal::units::Hertz;
use esp_idf_svc::sys::{EspError, ESP_ERR_INVALID_ARG, ESP_ERR_INVALID_STATE};
use powderbot_core::motor::{percent_to_hz, MAX_SPEED_HZ, MIN_SPEED_HZ};

#[derive(Clone, Copy, Debug)]
pub enum Direction {
    Forward,
    #[allow(dead_code)]
    Reverse,
}

pub struct Stepper {
    pwm: LedcDriver<'static>,
    timer: LedcTimerDriver<'static, LowSpeed>,
    direction: PinDriver<'static, Output>,
    enable: PinDriver<'static, Output>,
    speed_hz: u32,
    running: bool,
}

#[allow(dead_code)]
impl Stepper {
    pub fn new(
        timer: TIMER1<'static>,
        channel: CHANNEL1<'static>,
        step: Gpio33<'static>,
        direction: Gpio32<'static>,
        enable: Gpio25<'static>,
    ) -> Result<Self, EspError> {
        let mut enable = PinDriver::output(enable)?;
        enable.set_high()?;
        let direction = PinDriver::output(direction)?;
        // Initialize at the maximum rate so automatic clock selection can
        // support the full range (a low-rate reference clock may not).
        // Duty remains zero, so configuring this frequency causes no motion.
        let config = TimerConfig::new()
            .frequency(Hertz(MAX_SPEED_HZ))
            .resolution(Resolution::Bits14);
        let timer = LedcTimerDriver::new(timer, &config)?;
        let mut pwm = LedcDriver::new(channel, &timer, step)?;
        pwm.set_duty(0)?;
        let mut motor = Self {
            pwm,
            timer,
            direction,
            enable,
            speed_hz: 0,
            running: false,
        };
        motor.set_direction(Direction::Forward)?;
        Ok(motor)
    }

    pub fn set_speed_percent(&mut self, percent: f32) -> Result<(), EspError> {
        let hz = percent_to_hz(percent)
            .map_err(|_| EspError::from_infallible::<ESP_ERR_INVALID_ARG>())?;
        self.set_speed_hz(hz)
    }

    pub fn set_speed_hz(&mut self, hz: u32) -> Result<(), EspError> {
        if hz == 0 {
            self.stop()?;
            self.speed_hz = 0;
            return Ok(());
        }
        if !(MIN_SPEED_HZ..=MAX_SPEED_HZ).contains(&hz) {
            return Err(EspError::from_infallible::<ESP_ERR_INVALID_ARG>());
        }
        if let Err(error) = self.timer.set_frequency(Hertz(hz)) {
            self.stop_best_effort();
            return Err(error);
        }
        self.speed_hz = hz;
        Ok(())
    }

    pub fn set_direction(&mut self, direction: Direction) -> Result<(), EspError> {
        if self.running {
            return Err(EspError::from_infallible::<ESP_ERR_INVALID_STATE>());
        }
        match direction {
            Direction::Forward => self.direction.set_high(),
            Direction::Reverse => self.direction.set_low(),
        }
    }

    pub fn start(&mut self) -> Result<(), EspError> {
        if self.running {
            return Ok(());
        }
        if self.speed_hz == 0 {
            return Err(EspError::from_infallible::<ESP_ERR_INVALID_STATE>());
        }
        // Allow DIR to settle and the driver to enable before the first pulse.
        // These short setup delays occur only on start, never on stop.
        let result = (|| {
            thread::sleep(Duration::from_millis(1));
            self.enable.set_low()?;
            thread::sleep(Duration::from_millis(1));
            self.pwm.set_duty(self.pwm.get_max_duty().div_ceil(2))
        })();
        if result.is_err() {
            self.stop_best_effort();
        } else {
            self.running = true;
        }
        result
    }

    /// Disable the driver first, then silence STEP. No deceleration or pulse queue.
    /// Releases holding torque; mechanical inertia can still cause coast.
    pub fn stop(&mut self) -> Result<(), EspError> {
        let disabled = self.enable.set_high();
        let silenced = self.pwm.set_duty(0);
        if disabled.is_ok() && silenced.is_ok() {
            self.running = false;
        }
        disabled.and(silenced)
    }

    /// Commanded running state, not physical motion feedback.
    pub fn is_running(&self) -> bool {
        self.running
    }

    pub fn speed_hz(&self) -> u32 {
        self.speed_hz
    }

    fn stop_best_effort(&mut self) {
        if let Err(error) = self.stop() {
            log::error!("Motor stop failed: {error}");
        }
    }
}

impl Drop for Stepper {
    fn drop(&mut self) {
        self.stop_best_effort();
    }
}

#[cfg(feature = "motor-test")]
pub fn run_test(motor: &mut Stepper) -> Result<(), EspError> {
    log::warn!("MOTOR SWEEP: 10% to 100%, starting forward motion in 3 seconds");
    thread::sleep(Duration::from_secs(3));
    let result = (|| {
        motor.set_direction(Direction::Forward)?;
        motor.set_speed_percent(10.0)?;
        motor.start()?;
        for percent in 10..=100 {
            if percent > 10 {
                motor.set_speed_percent(percent as f32)?;
                thread::sleep(Duration::from_millis(20));
            }
            if percent % 10 == 0 {
                log::info!(
                    "Motor: {percent}% = {} Hz, running={}; holding 2 seconds",
                    motor.speed_hz(),
                    motor.is_running()
                );
                thread::sleep(Duration::from_secs(2));
            }
        }
        log::info!("Sweep reached 100%; stopping immediately");
        Ok(())
    })();
    // Attempt stop even when a preceding command failed.
    let stopped = motor.stop();
    result.and(stopped)?;
    log::info!("Motor sweep complete; driver disabled");
    Ok(())
}
