use powderbot_core::log_buffer::{LogBuffer, MAX_LINE_BYTES, MAX_LINES};

#[test]
fn retains_messages_in_order_without_consuming_them() {
    let mut buffer = LogBuffer::default();
    assert_eq!(buffer.snapshot(), "");
    buffer.push("Booting");
    buffer.push("Motor stopped");
    assert_eq!(buffer.snapshot(), "Booting\nMotor stopped\n");
    assert_eq!(buffer.snapshot(), "Booting\nMotor stopped\n");
}

#[test]
fn drops_oldest_messages_when_full() {
    let mut buffer = LogBuffer::default();
    for index in 0..MAX_LINES + 3 {
        buffer.push(&format!("message {index}"));
    }
    let snapshot = buffer.snapshot();
    assert_eq!(snapshot.lines().count(), MAX_LINES);
    assert_eq!(snapshot.lines().next(), Some("message 3"));
    let expected_last = format!("message {}", MAX_LINES + 2);
    assert_eq!(snapshot.lines().last(), Some(expected_last.as_str()));
}

#[test]
fn bounds_long_messages_without_breaking_unicode() {
    let mut buffer = LogBuffer::default();
    buffer.push(&"x".repeat(MAX_LINE_BYTES - 1));
    buffer.push(&format!("{}é", "x".repeat(MAX_LINE_BYTES - 1)));
    buffer.push(&"é".repeat(MAX_LINE_BYTES));
    let snapshot = buffer.snapshot();
    let lines: Vec<_> = snapshot.lines().collect();
    assert_eq!(lines[1], lines[0]);
    assert_eq!(lines[2].len(), MAX_LINE_BYTES);
    assert!(lines[2].chars().all(|character| character == 'é'));
}
