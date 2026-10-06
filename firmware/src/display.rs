
use embedded_graphics::{
    mono_font::{ascii::FONT_6X10, MonoTextStyle},
    pixelcolor::BinaryColor,
    prelude::*,
    text::{Baseline, Text},
};
use esp_idf_svc::hal::i2c::I2cDriver;
use ssd1306::{
    mode::BufferedGraphicsMode,
    prelude::{DisplayConfig, DisplayRotation, DisplaySize128x64, I2CInterface},
    I2CDisplayInterface, Ssd1306,
};

type Oled<'d> = Ssd1306<
    I2CInterface<I2cDriver<'d>>,
    DisplaySize128x64,
    BufferedGraphicsMode<DisplaySize128x64>,
>;

pub struct Display<'d> {
    oled: Oled<'d>,
}

impl<'d> Display<'d> {
    pub fn new(i2c: I2cDriver<'d>) -> Result<Self, String> {
        let interface = I2CDisplayInterface::new_custom_address(i2c, 0x3c);
        let mut oled = Ssd1306::new(interface, DisplaySize128x64, DisplayRotation::Rotate0)
            .into_buffered_graphics_mode();
        oled.init()
            .map_err(|error| format!("OLED init: {error:?}"))?;
        let mut display = Self { oled };
        display.screen("Starting...", "Waiting for scale")?;
        Ok(display)
    }

    pub fn show_weight(&mut self, weight: f32) -> Result<(), String> {
        let value = format!("{weight:.3} GN");
        // At 6 pixels per character, 21 characters fit on this display.
        let value = if value.len() <= 21 {
            value
        } else {
            format!("{weight:.3e} GN")
        };
        self.screen(&value, "Scale connected")
    }

    /// Replaces the weight completely, so an old value isn't shown as current.
    pub fn show_scale_error(&mut self, reason: &str) -> Result<(), String> {
        self.screen("Scale error", reason)
    }

    pub fn show_network(&mut self, address: &str) -> Result<(), String> {
        self.screen("WiFi: PowderBot", address)
    }

    fn screen(&mut self, value: &str, status: &str) -> Result<(), String> {
        self.oled.clear_buffer();
        let style = MonoTextStyle::new(&FONT_6X10, BinaryColor::On);

        for (text, y) in [("PowderBot", 0), (value, 22), (status, 42)] {
            Text::with_baseline(text, Point::new(0, y), style, Baseline::Top)
                .draw(&mut self.oled)
                .map_err(|error| format!("OLED draw: {error:?}"))?;
        }

        self.oled
            .flush()
            .map_err(|error| format!("OLED flush: {error:?}"))
    }
}
