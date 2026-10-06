
use esp_idf_svc::hal::gpio::{
    Gpio14, Gpio16, Gpio17, Gpio21, Gpio22, Gpio25, Gpio32, Gpio33, Pins,
};

pub struct BoardPins {
    pub stepper_step: Gpio33<'static>,
    pub stepper_dir: Gpio32<'static>,
    /// Active low: HIGH disables the motor driver.
    pub stepper_enable: Gpio25<'static>,
    pub oled_sda: Gpio21<'static>,
    pub oled_scl: Gpio22<'static>,
    pub scale_tx: Gpio16<'static>,
    pub scale_rx: Gpio17<'static>,
    pub beeper: Gpio14<'static>,
}

impl BoardPins {
    pub fn new(pins: Pins) -> Self {
        Self {
            stepper_step: pins.gpio33,
            stepper_dir: pins.gpio32,
            stepper_enable: pins.gpio25,
            oled_sda: pins.gpio21,
            oled_scl: pins.gpio22,
            scale_tx: pins.gpio16,
            scale_rx: pins.gpio17,
            beeper: pins.gpio14,
        }
    }
}
