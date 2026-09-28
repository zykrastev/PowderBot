//! One blocking request/reply transaction for the scale bring-up milestone.
//! Later, the controller will need a nonblocking polling state machine.

use std::fmt;
use std::thread;
use std::time::{Duration, Instant};

use esp_idf_svc::hal::{delay::NON_BLOCK, uart::UartDriver};
use esp_idf_svc::sys::{EspError, ESP_ERR_TIMEOUT};
use powderbot_core::scale_protocol::{parse_grains, ParseError};

const PRINT_COMMAND: [u8; 2] = [0x1b, 0x70];
const REPLY_TIMEOUT: Duration = Duration::from_millis(300);

#[derive(Debug)]
pub enum ScaleError {
    Uart(EspError),
    IncompleteWrite,
    Timeout,
    ReplyTooLong,
    InvalidUtf8,
    InvalidReply(ParseError),
}

impl fmt::Display for ScaleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Uart(error) => write!(f, "UART error: {error}"),
            Self::IncompleteWrite => write!(f, "print command was not fully written"),
            Self::Timeout => write!(f, "no complete reply within 300 ms"),
            Self::ReplyTooLong => write!(f, "reply exceeds the 32-byte buffer"),
            Self::InvalidUtf8 => write!(f, "reply is not valid UTF-8 text"),
            Self::InvalidReply(error) => write!(f, "invalid scale reply: {error:?}"),
        }
    }
}

/// Request one weight. Incomplete, oversized, and malformed replies are errors,
/// never replacement weights. This function borrows the UART driver.
pub fn read_weight(uart: &UartDriver<'_>) -> Result<f32, ScaleError> {
    // Discard bytes left from an earlier transaction, as the C++ firmware does.
    uart.clear_rx().map_err(ScaleError::Uart)?;
    let written = uart.write(&PRINT_COMMAND).map_err(ScaleError::Uart)?;
    if written != PRINT_COMMAND.len() {
        return Err(ScaleError::IncompleteWrite);
    }

    let started = Instant::now();
    let mut line = [0_u8; 32];
    let mut length = 0;

    while started.elapsed() < REPLY_TIMEOUT {
        let mut byte = [0_u8; 1];
        match uart.read(&mut byte, NON_BLOCK) {
            Ok(0) => thread::sleep(Duration::from_millis(5)),
            Ok(_) => match byte[0] {
                b'\n' => {
                    log::info!("Scale raw: {:?}", String::from_utf8_lossy(&line[..length]));
                    let text = std::str::from_utf8(&line[..length])
                        .map_err(|_| ScaleError::InvalidUtf8)?;
                    return parse_grains(text).map_err(ScaleError::InvalidReply);
                }
                value => {
                    if length == line.len() {
                        return Err(ScaleError::ReplyTooLong);
                    }
                    line[length] = value;
                    length += 1;
                }
            },
            Err(error) if error.code() == ESP_ERR_TIMEOUT => {
                // No data yet: yield CPU time and try again until our deadline.
                thread::sleep(Duration::from_millis(5));
            }
            Err(error) => return Err(ScaleError::Uart(error)),
        }
    }

    log::warn!("Scale partial bytes: {:02x?}", &line[..length]);
    Err(ScaleError::Timeout)
}
