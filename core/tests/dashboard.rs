use powderbot_core::dashboard::Dashboard;
use std::time::{Duration, Instant};

#[test]
fn missing_failed_and_stale_readings_are_not_zero_weights() {
    let now = Instant::now();
    let mut state = Dashboard::default();
    assert!(state.snapshot(now)["currentWeight"].is_null());
    state.record(Ok(12.5), now);
    assert_eq!(state.snapshot(now)["currentWeight"], 12.5);
    assert!(state.snapshot(now)["stable"].is_null());
    assert!(state.snapshot(now + Duration::from_secs(4))["currentWeight"].is_null());
    state.record(Err("No reply".into()), now);
    assert_eq!(state.snapshot(now)["scaleError"], "No reply");
    assert!(state.snapshot(now)["currentWeight"].is_null());
    state.record(Ok(13.0), now);
    assert!(state.snapshot(now)["scaleError"].is_null());
}

#[test]
fn settings_update_is_atomic_and_validates_each_supplied_field() {
    let now = Instant::now();
    let mut state = Dashboard::default();
    state.update_settings(br#"{"tolerance":0.03}"#).unwrap();
    assert!(
        state
            .update_settings(br#"{"targetWeight":20,"tolerance":-1}"#)
            .is_err()
    );
    assert_eq!(state.snapshot(now)["targetWeight"], 0.0);
    assert!(state.update_settings(br#"{"targetWeight":null}"#).is_err());
    assert!(state.update_settings(br#"{"typo":1}"#).is_err());
    assert!(state.update_settings(b"{}").is_err());
    assert!(state.update_settings(br#"{"targetWeight":1001}"#).is_err());
    state.update_settings(br#"{"targetWeight":20}"#).unwrap();
    state.record(Ok(12.5), now);
    assert_eq!(state.snapshot(now)["remainingWeight"], 7.5);
}
