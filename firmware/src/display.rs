
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

    pub fn show_status(
        &mut self,
        state: &powderbot_core::dashboard::Dashboard,
        now: std::time::Instant,
    ) -> Result<(), String> {
        let status = state.snapshot(now);
        let value = |key: &str| {
            status[key]
                .as_f64()
                .map(|v| format!("{v:.2}"))
                .unwrap_or_else(|| "--".into())
        };
        self.lines(&[
            "PowderBot".into(),
            format!("Powder: {}", state.profile.name),
            format!("Target: {} GN", value("targetWeight")),
            format!("Now: {} GN", value("currentWeight")),
            format!("Left: {} GN", value("remainingWeight")),
            if status["scaleConnected"].as_bool() == Some(true) {
                state.controller.state().name().into()
            } else {
                "Scale unavailable".into()
            },
        ])
    }
    pub fn show_network(&mut self, address: &str) -> Result<(), String> {
        self.lines(&[
            "WiFi: PowderBot".into(),
            "PW: powderbot".into(),
            address.into(),
            "Waiting for client".into(),
        ])
    }
    pub fn show_connected(&mut self) -> Result<(), String> {
        self.screen("WiFi", "Client connected")
    }
    fn lines(&mut self, lines: &[String]) -> Result<(), String> {
        self.oled.clear_buffer();
        let style = MonoTextStyle::new(&FONT_6X10, BinaryColor::On);
        for (i, line) in lines.iter().take(6).enumerate() {
            let text: String = line.chars().take(21).collect();
            Text::with_baseline(&text, Point::new(0, (i * 10) as i32), style, Baseline::Top)
                .draw(&mut self.oled)
                .map_err(|e| format!("OLED draw: {e:?}"))?;
        }
        self.oled.flush().map_err(|e| format!("OLED flush: {e:?}"))
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
