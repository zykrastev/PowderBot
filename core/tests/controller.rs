mod support;
use powderbot_core::controller::{Controller, Run, State};
use std::time::{Duration, Instant};
fn run() -> Run {
    Run {
        profile: support::profile(),
        target: 100.0,
        tolerance: 0.02,
    }
}
#[test]
fn phase_thresholds_and_hard_cutoff() {
    let t = Instant::now();
    let mut c = Controller::default();
    c.start(run(), Some((0.0, t)), t).unwrap();
    assert_eq!(c.state(), State::Coarse);
    c.tick(Some((98.0, t)), t);
    assert_eq!(c.state(), State::Trickling);
    assert_eq!(c.speed(), 50.0);
    c.tick(Some((99.5, t)), t);
    assert_eq!(c.state(), State::Settling);
    assert_eq!(c.speed(), 0.0);
}
#[test]
fn pulses_wait_for_time_and_a_new_reading_before_topping_up() {
    let t = Instant::now();
    let mut c = Controller::default();
    c.start(run(), Some((0.0, t)), t).unwrap();
    c.tick(Some((98.0, t)), t);
    c.tick(Some((98.0, t)), t + Duration::from_millis(99));
    assert_eq!(c.speed(), 50.0);
    c.tick(Some((98.0, t)), t + Duration::from_millis(100));
    assert_eq!(c.state(), State::Settling);
    let sample = t + Duration::from_millis(350);
    c.tick(Some((99.0, sample)), t + Duration::from_millis(400));
    assert_eq!(c.speed(), 0.0);
    let sample = t + Duration::from_millis(405);
    c.tick(Some((99.0, sample)), sample);
    assert_eq!(c.speed(), 50.0);
    let sample = t + Duration::from_millis(410);
    c.tick(Some((99.99, sample)), sample);
    assert_eq!(c.state(), State::Settling);
    let sample = t + Duration::from_millis(711);
    c.tick(Some((99.99, sample)), sample);
    assert_eq!(c.state(), State::Finished);
}
#[test]
fn stop_scale_loss_and_initial_limits_never_leave_motion_running() {
    let t = Instant::now();
    let mut c = Controller::default();
    assert!(c.start(run(), None, t).is_err());
    assert_eq!(c.speed(), 0.0);
    assert!(c.start(run(), Some((100.03, t)), t).is_err());
    assert!(c.start(run(), Some((100.0, t)), t).is_err());
    assert_eq!(c.speed(), 0.0);
    c.start(run(), Some((0.0, t)), t).unwrap();
    c.tick(None, t);
    assert_eq!(c.state(), State::Error);
    c.start(run(), Some((0.0, t)), t).unwrap();
    c.stop();
    assert_eq!(c.state(), State::Idle);
    c.start(run(), Some((0.0, t)), t).unwrap();
    c.tick(Some((0.0, t)), t + Duration::from_millis(401));
    assert_eq!(c.state(), State::Error);
}
#[test]
fn settings_validation_and_zero_speed_fail_closed() {
    let t = Instant::now();
    let mut c = Controller::default();
    let mut r = run();
    r.profile.coarse_speed_percent = 0.0;
    assert!(c.start(r, Some((0.0, t)), t).is_err());
    assert_eq!(c.speed(), 0.0);
    let mut r = run();
    r.target = f64::NAN;
    assert!(c.start(r, Some((0.0, t)), t).is_err());
}

#[test]
fn pulse_duration_begins_when_driver_starts() {
    let t = Instant::now();
    let mut c = Controller::default();
    c.start(run(), Some((0.0, t)), t).unwrap();
    c.tick(Some((98.0, t)), t);
    c.motion_started(t + Duration::from_millis(20));
    c.tick(Some((98.0, t)), t + Duration::from_millis(110));
    assert_eq!(c.state(), State::Trickling);
    c.tick(Some((98.0, t)), t + Duration::from_millis(120));
    assert_eq!(c.state(), State::Settling);
}

#[test]
fn start_requires_zero_within_tolerance_without_creating_a_finished_run() {
    let t = Instant::now();
    for weight in [0.020001, -0.020001, 50.0, 100.0, 100.03] {
        let mut c = Controller::default();
        assert!(
            c.start(run(), Some((weight, t)), t).is_err(),
            "weight {weight} should block Start"
        );
        assert_eq!(c.state(), State::Idle);
        assert_eq!(c.speed(), 0.0);
    }
    for weight in [-0.02, 0.0, 0.02] {
        let mut c = Controller::default();
        c.start(run(), Some((weight, t)), t).unwrap();
        assert_eq!(c.state(), State::Coarse);
    }
}
