use crate::scale_protocol::parse_grains;
use std::time::{Duration, Instant};
#[derive(Default)]
pub struct Poll {
    requested: Option<Instant>,
    last: Option<Instant>,
    line: [u8; 32],
    length: usize,
}
impl Poll {
    pub fn due(&self, now: Instant) -> bool {
        self.requested.is_none()
            && self
                .last
                .is_none_or(|t| now.saturating_duration_since(t) >= Duration::from_millis(100))
    }
    pub fn begin(&mut self, now: Instant) {
        self.requested = Some(now);
        self.last = Some(now);
        self.length = 0;
    }
    pub fn cancel(&mut self, now: Instant) {
        self.requested = None;
        self.last = Some(now);
        self.length = 0;
    }
    pub fn timeout(&mut self, now: Instant) -> Option<Result<f32, String>> {
        if self
            .requested
            .is_some_and(|t| now.saturating_duration_since(t) >= Duration::from_millis(300))
        {
            self.requested = None;
            Some(Err("No complete scale reply within 300 ms".into()))
        } else {
            None
        }
    }
    pub fn byte(&mut self, byte: u8) -> Option<Result<f32, String>> {
        self.requested?;
        if byte == b'\n' {
            self.requested = None;
            return Some(
                std::str::from_utf8(&self.line[..self.length])
                    .map_err(|_| {
                        format!(
                            "Invalid scale text: bytes={:02x?}",
                            &self.line[..self.length]
                        )
                    })
                    .and_then(|s| {
                        // The scale can echo the ESC p command before the weight.
                        let reply = s.strip_prefix("\u{1b}p").unwrap_or(s);
                        parse_grains(reply)
                            .map_err(|e| format!("Invalid scale reply: {e:?}; raw={s:?}"))
                    }),
            );
        }
        if self.length == self.line.len() {
            self.requested = None;
            return Some(Err("Scale reply exceeds 32 bytes".into()));
        }
        self.line[self.length] = byte;
        self.length += 1;
        None
    }
}
