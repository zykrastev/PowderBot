//! A read-only scale snapshot and volatile dashboard settings; no motor control.
use serde_json::{Value, json};
use std::time::{Duration, Instant};

pub struct Dashboard {
    weight: Option<f32>,
    sampled: Option<Instant>,
    error: Option<String>,
    target: f64,
    tolerance: f64,
}
impl Default for Dashboard {
    fn default() -> Self {
        Self {
            weight: None,
            sampled: None,
            error: None,
            target: 0.0,
            tolerance: 0.02,
        }
    }
}
impl Dashboard {
    pub fn record(&mut self, reading: Result<f32, String>, now: Instant) {
        match reading {
            Ok(weight) if weight.is_finite() => {
                self.weight = Some(weight);
                self.sampled = Some(now);
                self.error = None;
            }
            other => {
                self.weight = None;
                self.error = Some(other.err().unwrap_or_else(|| "Invalid weight".into()));
            }
        }
    }
    pub fn snapshot(&self, now: Instant) -> Value {
        let age = self
            .sampled
            .map(|sampled| now.saturating_duration_since(sampled));
        let fresh = self.weight.is_some() && age.is_some_and(|age| age <= Duration::from_secs(3));
        let weight = self.weight.filter(|_| fresh);
        let error = self.error.clone().or_else(|| {
            if fresh {
                None
            } else {
                Some("Waiting for a fresh scale reading".into())
            }
        });
        json!({
            "currentWeight": weight, "remainingWeight": weight.map(|w| self.target - f64::from(w)),
            "targetWeight":self.target, "tolerance":self.tolerance,
            "stable":null, "scaleConnected":fresh, "scaleError":error,
            "sampleAgeMs":age.map(|age| age.as_millis().min(u64::MAX as u128) as u64),
            "state":if fresh { "Idle" } else { "ScaleError" },
            "capabilities":{"dispensing":false,"tare":false,"reset":false}
        })
    }
    pub fn update_settings(&mut self, bytes: &[u8]) -> Result<(), String> {
        let value: Value = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
        let fields = value.as_object().ok_or("Settings must be an object")?;
        if fields.is_empty()
            || fields
                .keys()
                .any(|key| key != "targetWeight" && key != "tolerance")
        {
            return Err("Supply targetWeight and/or tolerance".into());
        }
        let mut target = self.target;
        let mut tolerance = self.tolerance;
        for (key, value) in fields {
            let value = value
                .as_f64()
                .filter(|n| n.is_finite() && *n > 0.0 && *n <= 1000.0)
                .ok_or("Values must be numbers greater than 0 and at most 1000 grains")?;
            if key == "targetWeight" {
                target = value;
            } else {
                tolerance = value;
            }
        }
        self.target = target;
        self.tolerance = tolerance;
        Ok(())
    }
}
