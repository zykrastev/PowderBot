mod support;
use powderbot_core::profile::{Profile, validate_storage_name};
use serde_json::json;

#[test]
fn modern_round_trip_and_legacy_conversion() {
    let p = support::profile();
    assert_eq!(Profile::from_json(&p.to_json().unwrap()).unwrap(), p);
    let legacy = br#"{"version":1,"id":"p1","name":"Test powder","coarseSpeed":100,"fineSpeed":30,"trickleSpeed":5}"#;
    assert_eq!(Profile::from_json(legacy).unwrap(), p);
    let mut value: serde_json::Value = serde_json::from_slice(legacy).unwrap();
    value["settleTimeMs"] = json!(300);
    assert!(Profile::from_json(&serde_json::to_vec(&value).unwrap()).is_err());
}

#[test]
fn rejects_incomplete_or_wrong_types() {
    let p = support::profile();
    let value: serde_json::Value = serde_json::from_slice(&p.to_json().unwrap()).unwrap();
    for key in value.as_object().unwrap().keys() {
        let mut bad = value.clone();
        bad.as_object_mut().unwrap().remove(key);
        assert!(
            Profile::from_json(&serde_json::to_vec(&bad).unwrap()).is_err(),
            "{key}"
        );
    }
    for (key, bad_value) in [
        ("version", json!(2)),
        ("settleTimeMs", json!(-1)),
        ("settleTimeMs", json!(1.5)),
        ("settleTimeMs", json!(4294967296_u64)),
        ("fineSpeedPercent", json!("30")),
    ] {
        let mut bad = value.clone();
        bad[key] = bad_value;
        assert!(Profile::from_json(&serde_json::to_vec(&bad).unwrap()).is_err());
    }
}

#[test]
fn enforces_names_and_numeric_bounds() {
    for name in [
        "",
        "../x",
        "a/b",
        "a\\b",
        "é",
        "active_profile",
        "storage_probe",
    ] {
        assert!(validate_storage_name(name).is_err());
    }
    assert!(validate_storage_name(&"x".repeat(26)).is_ok());
    assert!(validate_storage_name(&"x".repeat(27)).is_err());
    let mut p = support::profile();
    p.id = "x".repeat(24);
    p.name = "é".repeat(32);
    assert!(p.validate().is_ok());
    p.id.push('x');
    assert!(p.validate().is_err());
    p.id = "valid_-9".into();
    p.name.push('x');
    assert!(p.validate().is_err());
    p.name = "Powder".into();
    for n in [-1.0, 100.1, f32::NAN, f32::INFINITY] {
        p.coarse_speed_percent = n;
        assert!(p.validate().is_err());
    }
    p.coarse_speed_percent = 0.0;
    p.fine_start_percent = 98.0;
    assert!(p.validate().is_err());
    p.fine_start_percent = 97.0;
    p.stop_percent = 96.0;
    assert!(p.validate().is_err());
    p.stop_percent = 100.0;
    assert!(p.validate().is_ok());
}
