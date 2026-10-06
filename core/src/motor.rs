
pub const MAX_SPEED_HZ: u32 = 2667;
pub const MIN_SPEED_HZ: u32 = 10;

#[derive(Debug, PartialEq, Eq)]
pub struct InvalidSpeed;

pub fn percent_to_hz(percent: f32) -> Result<u32, InvalidSpeed> {
    if !percent.is_finite() || !(0.0..=100.0).contains(&percent) {
        return Err(InvalidSpeed);
    }
    if percent == 0.0 {
        return Ok(0);
    }
    Ok(((MAX_SPEED_HZ as f32 * percent / 100.0).round() as u32).max(MIN_SPEED_HZ))
}
