use crate::profile::Profile;
use std::time::{Duration, Instant};

pub const FRESHNESS: Duration = Duration::from_millis(400);

pub fn weight_is_zero(weight: f64, tolerance: f64) -> bool {
    // Scale readings originate as f32. Allow only their representation error
    // at the inclusive tolerance boundary (e.g. a reading of 0.05 grains).
    weight.is_finite()
        && tolerance.is_finite()
        && tolerance > 0.0
        && weight.abs() <= tolerance + tolerance * f64::from(f32::EPSILON)
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    Idle,
    Coarse,
    Fine,
    Trickling,
    Settling,
    Finished,
    Overthrown,
    Error,
}
impl State {
    pub fn active(self) -> bool {
        matches!(
            self,
            Self::Coarse | Self::Fine | Self::Trickling | Self::Settling
        )
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::Idle => "Idle",
            Self::Coarse => "Coarse",
            Self::Fine => "Fine",
            Self::Trickling => "Trickling",
            Self::Settling => "Settling",
            Self::Finished => "Finished",
            Self::Overthrown => "Overthrown",
            Self::Error => "Error",
        }
    }
}
#[derive(Clone)]
pub struct Run {
    pub profile: Profile,
    pub target: f64,
    pub tolerance: f64,
}
#[derive(Clone)]
pub struct Controller {
    state: State,
    run: Option<Run>,
    deadline: Option<Instant>,
    topping_up: bool,
    error: Option<String>,
}
impl Default for Controller {
    fn default() -> Self {
        Self {
            state: State::Idle,
            run: None,
            deadline: None,
            topping_up: false,
            error: None,
        }
    }
}
impl Controller {
    pub fn state(&self) -> State {
        self.state
    }
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }
    pub fn stop(&mut self) {
        self.state = State::Idle;
        self.deadline = None;
        self.topping_up = false;
        self.error = None;
    }
    pub fn fail(&mut self, message: impl Into<String>) {
        self.state = State::Error;
        self.deadline = None;
        self.error = Some(message.into());
    }
    pub fn speed(&self) -> f32 {
        match (self.state, self.run.as_ref()) {
            (State::Coarse, Some(run)) => run.profile.coarse_speed_percent,
            (State::Fine, Some(run)) => run.profile.fine_speed_percent,
            (State::Trickling, Some(run)) => run.profile.trickle_speed_percent,
            _ => 0.0,
        }
    }
    /// Called after the driver actually begins emitting pulses.
    pub fn motion_started(&mut self, now: Instant) {
        if self.state == State::Trickling {
            self.deadline = Some(
                now + Duration::from_millis(u64::from(
                    self.run.as_ref().unwrap().profile.trickle_pulse_time_ms,
                )),
            );
        }
    }
    pub fn start(
        &mut self,
        run: Run,
        reading: Option<(f64, Instant)>,
        now: Instant,
    ) -> Result<(), String> {
        if self.state.active() {
            return Err("Already dispensing".into());
        }
        let valid = run.profile.validate().is_ok()
            && [run.target, run.tolerance]
                .iter()
                .all(|n| n.is_finite() && *n > 0.0 && *n <= 1000.0);
        let Some((weight, _)) = reading
            .filter(|(w, t)| w.is_finite() && now.saturating_duration_since(*t) <= FRESHNESS)
        else {
            self.fail("A fresh scale reading is required");
            return Err(self.error.clone().unwrap());
        };
        if !valid {
            self.fail("Invalid dispensing settings");
            return Err(self.error.clone().unwrap());
        }
        if !weight_is_zero(weight, run.tolerance) {
            return Err(format!(
                "Empty and tare the scale before starting (weight must be 0 ± {} gr)",
                run.tolerance
            ));
        }
        self.error = None;
        self.topping_up = weight >= run.target * f64::from(run.profile.stop_percent) / 100.0;
        self.run = Some(run);
        let run = self.run.as_ref().unwrap();
        if weight > run.target + run.tolerance {
            self.state = State::Overthrown;
        } else if weight >= run.target - run.tolerance {
            self.state = State::Finished;
        } else {
            self.enter_phase(weight, now);
        }
        if self.state == State::Error {
            Err(self.error.clone().unwrap())
        } else {
            Ok(())
        }
    }
    fn enter_phase(&mut self, weight: f64, now: Instant) {
        let run = self.run.as_ref().unwrap();
        self.state = if self.topping_up {
            State::Trickling
        } else if weight < run.target * f64::from(run.profile.fine_start_percent) / 100.0 {
            State::Coarse
        } else if weight < run.target * f64::from(run.profile.trickle_start_percent) / 100.0 {
            State::Fine
        } else {
            State::Trickling
        };
        self.deadline = if self.state == State::Trickling {
            Some(now + Duration::from_millis(u64::from(run.profile.trickle_pulse_time_ms)))
        } else {
            None
        };
        if self.speed() <= 0.0 {
            self.fail("Moving phase has zero speed");
        }
    }
    fn settle(&mut self, now: Instant) {
        self.state = State::Settling;
        self.deadline = Some(
            now + Duration::from_millis(u64::from(
                self.run.as_ref().unwrap().profile.settle_time_ms,
            )),
        );
    }
    pub fn tick(&mut self, reading: Option<(f64, Instant)>, now: Instant) {
        if !self.state.active() {
            return;
        }
        let Some((weight, sampled)) = reading
            .filter(|(w, t)| w.is_finite() && now.saturating_duration_since(*t) <= FRESHNESS)
        else {
            self.fail("Scale reading lost or stale");
            return;
        };
        let run = self.run.as_ref().unwrap();
        if self.state == State::Settling {
            let deadline = self.deadline.unwrap();
            if now < deadline || sampled <= deadline {
                return;
            }
            if weight > run.target + run.tolerance {
                self.state = State::Overthrown;
            } else if weight >= run.target - run.tolerance {
                self.state = State::Finished;
            } else {
                self.topping_up = true;
                self.enter_phase(weight, now);
            }
            return;
        }
        let cutoff = if self.topping_up {
            run.target - run.tolerance
        } else {
            run.target * f64::from(run.profile.stop_percent) / 100.0
        };
        if weight >= cutoff || (self.state == State::Trickling && now >= self.deadline.unwrap()) {
            self.settle(now);
            return;
        }
        // Phase transitions only advance; noisy readings never restart coarse dosing.
        if self.state == State::Coarse
            && weight >= run.target * f64::from(run.profile.fine_start_percent) / 100.0
            || self.state == State::Fine
                && weight >= run.target * f64::from(run.profile.trickle_start_percent) / 100.0
        {
            self.enter_phase(weight, now);
        }
    }
}
