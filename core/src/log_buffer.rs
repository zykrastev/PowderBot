
use std::collections::VecDeque;

pub const MAX_LINES: usize = 96;
pub const MAX_LINE_BYTES: usize = 384;

#[derive(Default)]
pub struct LogBuffer {
    lines: VecDeque<String>,
}

impl LogBuffer {
    pub fn push(&mut self, message: &str) {
        let mut end = message.len().min(MAX_LINE_BYTES);
        while !message.is_char_boundary(end) {
            end -= 1;
        }
        if self.lines.len() == MAX_LINES {
            self.lines.pop_front();
        }
        self.lines.push_back(message[..end].to_owned());
    }

    pub fn snapshot(&self) -> String {
        let mut text = String::new();
        for line in &self.lines {
            text.push_str(line);
            text.push('\n');
        }
        text
    }
}
