use serde_json::{Value, json};
use std::time::{Duration, Instant};

#[derive(Clone)]
pub struct Dashboard {
    pub controller: crate::controller::Controller,
    pub profile: crate::profile::Profile,
    pub storage_name: String,
    stable_since: Option<Instant>,
    weight: Option<f32>,
    sampled: Option<Instant>,
    error: Option<String>,
    target: f64,
    tolerance: f64,
}
impl Default for Dashboard {
    fn default() -> Self {
        Self {
            controller: Default::default(),
            profile: Default::default(),
            storage_name: String::new(),
            stable_since: None,
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
                if self.weight.is_none_or(|old| (old - weight).abs() > 0.005)
                    || self.sampled.is_none_or(|time| {
                        now.saturating_duration_since(time) > crate::controller::FRESHNESS
                    })
                {
                    self.stable_since = Some(now);
                }
                self.weight = Some(weight);
                self.sampled = Some(now);
                self.error = None;
            }
            other => {
                self.stable_since = None;
                self.weight = None;
                self.error = Some(other.err().unwrap_or_else(|| "Invalid weight".into()));
            }
        }
    }
    pub fn snapshot(&self, now: Instant) -> Value {
        let age = self
            .sampled
            .map(|sampled| now.saturating_duration_since(sampled));
        let fresh =
            self.weight.is_some() && age.is_some_and(|age| age <= crate::controller::FRESHNESS);
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
            "stable":fresh && self.stable_since.is_some_and(|time| now.saturating_duration_since(time)>=Duration::from_secs(1)), "scaleConnected":fresh, "scaleError":error,
            "sampleAgeMs":age.map(|age| age.as_millis().min(u64::MAX as u128) as u64),
            "state":self.controller.state().name(), "controllerError":self.controller.error(),
            "powder":self.profile, "powderStorageName":self.storage_name,
            "motorSpeedPercent":self.controller.speed(), "effectiveTrickleSpeedPercent":50,
            "dispensing":self.controller.state().active(),
            "capabilities":{"dispensing":true,"tare":true,"reset":true}
        })
    }
    pub fn reading(&self, now: Instant) -> Option<(f64, Instant)> {
        self.weight
            .zip(self.sampled)
            .filter(|(_, t)| now.saturating_duration_since(*t) <= crate::controller::FRESHNESS)
            .map(|(w, t)| (f64::from(w), t))
    }
    pub fn tick(&mut self, now: Instant) {
        self.controller.tick(self.reading(now), now);
    }
    pub fn start(&mut self, now: Instant) -> Result<(), String> {
        self.controller.start(
            crate::controller::Run {
                profile: self.profile.clone(),
                target: self.target,
                tolerance: self.tolerance,
            },
            self.reading(now),
            now,
        )
    }
    pub fn invalidate(&mut self) {
        self.weight = None;
        self.sampled = None;
        self.stable_since = None;
        self.error = None;
    }
    pub fn select(
        &mut self,
        selected: Option<crate::profile_store::StoredProfile>,
    ) -> Result<(), String> {
        if self.controller.state().active() {
            return Err("Stop dispensing before changing profiles".into());
        }
        if let Some(selected) = selected {
            self.profile = selected.profile;
            self.storage_name = selected.storage_name;
        } else {
            self.profile = Default::default();
            self.storage_name.clear();
        }
        Ok(())
    }
    pub fn update_settings(&mut self, bytes: &[u8]) -> Result<(), String> {
        if self.controller.state().active() {
            return Err("Stop dispensing before changing settings".into());
        }
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
