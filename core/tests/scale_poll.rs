use powderbot_core::scale_poll::Poll;
use std::time::{Duration, Instant};
#[test]
fn fragmented_reply_timeout_and_tare_cancellation() {
    let t = Instant::now();
    let mut p = Poll::default();
    assert!(p.due(t));
    p.begin(t);
    for b in b" 1.00 GN" {
        assert!(p.byte(*b).is_none());
    }
    assert_eq!(p.byte(b'\n').unwrap().unwrap(), 1.0);
    assert!(!p.due(t + Duration::from_millis(99)));
    p.begin(t + Duration::from_millis(100));
    assert!(p.timeout(t + Duration::from_millis(399)).is_none());
    assert!(p.timeout(t + Duration::from_millis(400)).unwrap().is_err());
    p.begin(t);
    p.cancel(t);
    assert!(p.byte(b'\n').is_none());
}
#[test]
fn invalid_and_oversized_lines_never_produce_weights() {
    let t = Instant::now();
    let mut p = Poll::default();
    p.begin(t);
    for _ in 0..32 {
        assert!(p.byte(b'1').is_none());
    }
    assert!(p.byte(b'1').unwrap().is_err());
    assert!(p.byte(b'\n').is_none());
    p.begin(t);
    for b in b"1.0 XX" {
        p.byte(*b);
    }
    assert!(p.byte(b'\n').unwrap().is_err());
}
