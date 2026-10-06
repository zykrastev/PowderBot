use powderbot_core::dashboard::Dashboard;
use std::time::{Duration, Instant};

#[test]
fn missing_failed_and_stale_readings_are_not_zero_weights() {
    let now = Instant::now();
    let mut state = Dashboard::default();
    assert!(state.snapshot(now)["currentWeight"].is_null());
    state.record(Ok(12.5), now);
    assert_eq!(state.snapshot(now)["currentWeight"], 12.5);
    assert_eq!(state.snapshot(now)["stable"], false);
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

#[test]
fn stability_and_tare_require_new_readings() {
    let t = Instant::now();
    let mut s = Dashboard::default();
    for i in 0..=10 {
        s.record(Ok(1.0), t + Duration::from_millis(i * 100));
    }
    assert_eq!(s.snapshot(t + Duration::from_secs(1))["stable"], true);
    s.record(Ok(1.01), t + Duration::from_millis(1100));
    assert_eq!(s.snapshot(t + Duration::from_millis(1100))["stable"], false);
    s.invalidate();
    assert!(s.reading(t + Duration::from_millis(1100)).is_none());
}

#[test]
fn load_history_records_final_results_once_and_keeps_run_settings() {
    let t = Instant::now();
    let mut s = Dashboard::default();
    s.profile.name = "Test powder".into();
    s.update_settings(br#"{"targetWeight":20,"tolerance":0.02}"#)
        .unwrap();
    s.record(Ok(0.0), t);
    s.start(t).unwrap();
    s.record(Ok(20.0), t + Duration::from_millis(100));
    s.tick(t + Duration::from_millis(100));
    s.record(Ok(20.01), t + Duration::from_millis(500));
    s.tick(t + Duration::from_millis(500));
    s.tick(t + Duration::from_millis(510));
    let snapshot = history(&s, t + Duration::from_millis(510));
    let rows = snapshot["loads"].as_array().unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["accepted"], true);
    assert_eq!(rows[0]["targetWeight"], 20.0);
    assert!((rows[0]["measuredWeight"].as_f64().unwrap() - 20.01).abs() < 0.001);
    assert_eq!(rows[0]["powderName"], "Test powder");
    assert!(rows[0]["uptimeMs"].as_u64().is_some());
    s.update_settings(br#"{"targetWeight":21}"#).unwrap();
    s.record(Ok(99.0), t + Duration::from_millis(520));
    assert_eq!(history(&s, t)["loads"][0], rows[0]);
}

#[test]
fn history_logs_overthrows_and_reset_starts_new_series() {
    let t = Instant::now();
    let mut s = Dashboard::default();
    s.update_settings(br#"{"targetWeight":20}"#).unwrap();
    let t = complete_load(&mut s, t, 20.1);
    assert!(s.start(t).is_err());
    let t = complete_load(&mut s, t, 20.1);
    assert_eq!(history(&s, t)["loads"].as_array().unwrap().len(), 2);
    assert_eq!(history(&s, t)["loads"][0]["accepted"], false);
    s.clear_load_history().unwrap();
    s.tick(t);
    assert_eq!(history(&s, t)["loads"], serde_json::json!([]));
    let t = complete_load(&mut s, t, 20.0);
    assert_eq!(history(&s, t)["loads"][0]["id"], 1);
}

#[test]
fn history_is_bounded_and_does_not_record_cancelled_or_failed_loads() {
    let t = Instant::now();
    let mut s = Dashboard::default();
    s.update_settings(br#"{"targetWeight":20}"#).unwrap();
    s.record(Ok(0.0), t);
    s.start(t).unwrap();
    assert!(s.clear_load_history().is_err());
    s.controller.stop();
    s.tick(t);
    s.start(t).unwrap();
    s.tick(t + Duration::from_secs(1));
    assert_eq!(history(&s, t)["loads"], serde_json::json!([]));
    let mut t = t + Duration::from_secs(2);
    for _ in 0..205 {
        t = complete_load(&mut s, t, 20.0);
    }
    let history = history(&s, t);
    let rows = history["loads"].as_array().unwrap();
    assert_eq!(rows.len(), 200);
    assert_eq!(rows[0]["id"], 6);
    assert_eq!(rows[199]["id"], 205);
}

#[test]
fn overthrow_warning_survives_stop_and_clears_on_reset_or_next_start() {
    let t = Instant::now();
    let mut s = Dashboard::default();
    s.update_settings(br#"{"targetWeight":20}"#).unwrap();
    let t = complete_load(&mut s, t, 21.0);
    assert_eq!(s.snapshot(t)["overthrowAlert"], true);
    s.controller.stop();
    assert_eq!(s.snapshot(t)["overthrowAlert"], true);
    s.invalidate();
    assert_eq!(s.snapshot(t)["overthrowAlert"], false);
    let t = complete_load(&mut s, t, 21.0);
    s.record(Ok(0.0), t);
    s.start(t).unwrap();
    assert_eq!(s.snapshot(t)["overthrowAlert"], false);
}

fn history(state: &Dashboard, now: Instant) -> serde_json::Value {
    serde_json::from_slice(&state.load_history(now).unwrap()).unwrap()
}

fn complete_load(state: &mut Dashboard, t: Instant, weight: f32) -> Instant {
    state.record(Ok(0.0), t);
    state.start(t).unwrap();
    state.record(Ok(weight), t + Duration::from_millis(100));
    state.tick(t + Duration::from_millis(100));
    let done = t + Duration::from_millis(500);
    state.record(Ok(weight), done);
    state.tick(done);
    done
}

#[test]
fn nonzero_start_is_rejected_without_logging_and_ui_readiness_follows_scale() {
    let t = Instant::now();
    let mut s = Dashboard::default();
    s.update_settings(br#"{"targetWeight":20,"tolerance":0.05}"#)
        .unwrap();
    assert_eq!(s.snapshot(t)["canStart"], false);
    for weight in [1.0, -1.0, 20.0, 21.0] {
        s.record(Ok(weight), t);
        assert_eq!(s.snapshot(t)["canStart"], false);
        assert!(s.start(t).is_err());
        assert_eq!(history(&s, t)["loads"], serde_json::json!([]));
        assert_eq!(s.snapshot(t)["loadRevision"], 0);
    }
    for weight in [-0.05, 0.0, 0.05] {
        s.record(Ok(weight), t);
        assert_eq!(s.snapshot(t)["canStart"], true, "boundary {weight}");
        s.start(t).unwrap();
        assert_eq!(s.snapshot(t)["canStart"], false);
        s.controller.stop();
    }
    assert_eq!(s.snapshot(t + Duration::from_secs(1))["canStart"], false);
}
