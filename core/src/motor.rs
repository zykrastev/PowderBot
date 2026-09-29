//! Motor speed policy, independent of ESP32 hardware.

pub const MAX_SPEED_HZ: u32 = 2667;
// With a 14-bit LEDC timer, this range fits the classic ESP32 APB clock.
pub const MIN_SPEED_HZ: u32 = 10;

#[derive(Debug, PartialEq, Eq)]
pub struct InvalidSpeed;

/// Zero means stopped. Positive speeds have a 10 Hz floor. Invalid input is
/// rejected rather than silently accepting a negative or non-finite speed.
pub fn percent_to_hz(percent: f32) -> Result<u32, InvalidSpeed> {
    if !percent.is_finite() || !(0.0..=100.0).contains(&percent) {
        return Err(InvalidSpeed);
    }
    if percent == 0.0 {
        return Ok(0);
    }
    Ok(((MAX_SPEED_HZ as f32 * percent / 100.0).round() as u32).max(MIN_SPEED_HZ))
}
