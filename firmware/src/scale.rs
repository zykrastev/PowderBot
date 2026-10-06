use esp_idf_svc::{
    hal::{delay::NON_BLOCK, uart::UartDriver},
    sys::ESP_ERR_TIMEOUT,
};
use powderbot_core::scale_poll::Poll;
use std::time::Instant;
pub struct Scale {
    uart: UartDriver<'static>,
    poll: Poll,
}
impl Scale {
    pub fn new(uart: UartDriver<'static>) -> Self {
        Self {
            uart,
            poll: Poll::default(),
        }
    }
    pub fn update(&mut self, now: Instant) -> Option<Result<f32, String>> {
        if let Some(result) = self.poll.timeout(now) {
            return Some(result);
        }
        if self.poll.due(now) {
            self.poll.begin(now);
            if let Err(error) = self.send(&[0x1b, 0x70]) {
                self.poll.cancel(now);
                return Some(Err(error));
            }
        }
        let mut bytes = [0u8; 32];
        match self.uart.read(&mut bytes, NON_BLOCK) {
            Ok(count) => {
                for &byte in &bytes[..count] {
                    if let Some(result) = self.poll.byte(byte) {
                        return Some(result);
                    }
                }
                None
            }
            Err(error) if error.code() == ESP_ERR_TIMEOUT => None,
            Err(error) => {
                self.poll.cancel(now);
                Some(Err(error.to_string()))
            }
        }
    }
    fn send(&self, command: &[u8]) -> Result<(), String> {
        self.uart.clear_rx().map_err(|e| e.to_string())?;
        let count = self.uart.write(command).map_err(|e| e.to_string())?;
        if count == command.len() {
            Ok(())
        } else {
            Err("Incomplete scale command write".into())
        }
    }
    pub fn tare(&mut self, now: Instant) -> Result<(), String> {
        self.poll.cancel(now);
        self.send(&[0x1b, 0x74])
    }
}
