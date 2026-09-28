use powderbot_core::scale_protocol::{ParseError, parse_grains};

#[test]
fn parses_positive_weight_and_zero() {
    assert_eq!(parse_grains("12.50 GN"), Ok(12.5));
    assert_eq!(parse_grains("0 GN"), Ok(0.0));
}

#[test]
fn parses_attached_and_padded_signs() {
    assert_eq!(parse_grains("-0.10 GN"), Ok(-0.1));
    assert_eq!(parse_grains("-    0.10 GN"), Ok(-0.1));
    assert_eq!(parse_grains("+    0.10 GN"), Ok(0.1));
}

#[test]
fn accepts_padding_and_serial_line_endings() {
    assert_eq!(parse_grains("   12.50   GN\r\n"), Ok(12.5));
}

#[test]
fn rejects_empty_replies() {
    for reply in ["", "   ", "\r\n"] {
        assert_eq!(parse_grains(reply), Err(ParseError::EmptyReply));
    }
}

#[test]
fn rejects_missing_fields_and_multiple_lines() {
    for reply in ["12.50", "GN", "12.50GN", "12.50\nGN"] {
        assert_eq!(parse_grains(reply), Err(ParseError::InvalidFormat));
    }
}

#[test]
fn rejects_units_other_than_grains() {
    for reply in ["12.50 g", "12.50 mg", "12.50 gn", "12.50 OZ"] {
        assert_eq!(parse_grains(reply), Err(ParseError::UnsupportedUnit));
    }
}

#[test]
fn rejects_malformed_and_non_finite_weights() {
    for reply in [
        "abc GN",
        "12abc GN",
        "1 2.50 GN",
        "--0.10 GN",
        "+-0.10 GN",
        "- GN",
        ". GN",
        "1.2.3 GN",
        "NaN GN",
        "inf GN",
        "1e3 GN",
        "99999999999999999999999999999999999999999999999999 GN",
    ] {
        assert_eq!(
            parse_grains(reply),
            Err(ParseError::InvalidWeight),
            "{reply:?}"
        );
    }
}
