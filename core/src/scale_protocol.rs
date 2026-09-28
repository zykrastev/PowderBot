//! Parsing for the scale's text replies. UART framing and timeouts belong
//! in the firmware; this module receives one complete line at a time.

/// Why a reply could not be interpreted as a weight in grains.
#[derive(Debug, PartialEq, Eq)]
pub enum ParseError {
    EmptyReply,
    InvalidFormat,
    UnsupportedUnit,
    InvalidWeight,
}

/// Parse a decimal reading such as `"-    0.10 GN\r\n"` into grains.
///
/// `GN` is the only accepted unit. No unit conversion is performed.
/// Leading/trailing ASCII whitespace and padding after a sign are accepted;
/// embedded line endings, exponent notation, and non-finite values are not.
/// This format is based on the C++ example, pending real scale captures.
pub fn parse_grains(reply: &str) -> Result<f32, ParseError> {
    let line = reply.trim_ascii();
    if line.is_empty() {
        return Err(ParseError::EmptyReply);
    }
    if line.contains(['\r', '\n']) {
        return Err(ParseError::InvalidFormat);
    }

    let (number, unit) = line
        .rsplit_once(|c: char| c.is_ascii_whitespace())
        .ok_or(ParseError::InvalidFormat)?;
    if unit != "GN" {
        return Err(ParseError::UnsupportedUnit);
    }

    let number = number.trim_ascii();
    let (sign, magnitude) = match number.as_bytes().first() {
        Some(b'-') => (-1.0, number[1..].trim_ascii_start()),
        Some(b'+') => (1.0, number[1..].trim_ascii_start()),
        _ => (1.0, number),
    };

    // Validate the decimal grammar before parsing. In particular, don't
    // silently turn a broken reading such as "1 2.50" into "12.50".
    let mut digits = 0;
    let mut dots = 0;
    for byte in magnitude.bytes() {
        match byte {
            b'0'..=b'9' => digits += 1,
            b'.' => dots += 1,
            _ => return Err(ParseError::InvalidWeight),
        }
    }
    if digits == 0 || dots > 1 {
        return Err(ParseError::InvalidWeight);
    }

    let weight = magnitude
        .parse::<f32>()
        .map_err(|_| ParseError::InvalidWeight)?;
    if !weight.is_finite() {
        return Err(ParseError::InvalidWeight);
    }

    Ok(sign * weight)
}
