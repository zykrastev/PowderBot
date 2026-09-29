use powderbot_core::motor::{InvalidSpeed, percent_to_hz};

#[test]
fn zero_requests_a_stop() {
    assert_eq!(percent_to_hz(0.0), Ok(0));
}

#[test]
fn converts_test_and_profile_speeds() {
    assert_eq!(percent_to_hz(5.0), Ok(133));
    assert_eq!(percent_to_hz(10.0), Ok(267));
    assert_eq!(percent_to_hz(20.0), Ok(533));
    assert_eq!(percent_to_hz(30.0), Ok(800));
    assert_eq!(percent_to_hz(100.0), Ok(2667));
}

#[test]
fn tiny_positive_speeds_use_the_hardware_floor() {
    assert_eq!(percent_to_hz(0.01), Ok(10));
}

#[test]
fn rejects_invalid_speed_commands() {
    for value in [-1.0, 100.1, f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        assert_eq!(percent_to_hz(value), Err(InvalidSpeed));
    }
}
