
#[derive(Debug, PartialEq, Eq)]
pub enum ParseError {
    EmptyReply,
    InvalidFormat,
    UnsupportedUnit,
    InvalidWeight,
}

// Unitless replies assume the scale remains set to GN.
pub fn parse_grains(reply: &str) -> Result<f32, ParseError> {
    let line = reply.trim_ascii();
    if line.is_empty() {
        return Err(ParseError::EmptyReply);
    }
    if line.contains(['\r', '\n']) {
        return Err(ParseError::InvalidFormat);
    }

    let number = match line.rsplit_once(|c: char| c.is_ascii_whitespace()) {
        Some((number, "GN")) => number,
        Some((_, unit)) if unit.bytes().all(|byte| byte.is_ascii_alphabetic()) => {
            return Err(ParseError::UnsupportedUnit);
        }
        None if line.bytes().any(|byte| byte.is_ascii_alphabetic()) => {
            return Err(ParseError::InvalidFormat);
        }
        _ => line,
    };

    let number = number.trim_ascii();
    let (sign, magnitude) = match number.as_bytes().first() {
        Some(b'-') => (-1.0, number[1..].trim_ascii_start()),
        Some(b'+') => (1.0, number[1..].trim_ascii_start()),
        _ => (1.0, number),
    };

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
