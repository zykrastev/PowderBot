//! The C++ version-1 JSON format, independent of hardware and storage.
use serde::{Deserialize, Serialize};
use std::{error::Error, fmt};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub version: u8,
    pub id: String,
    pub name: String,
    pub fine_start_percent: f32,
    pub trickle_start_percent: f32,
    pub stop_percent: f32,
    pub coarse_speed_percent: f32,
    pub fine_speed_percent: f32,
    pub trickle_speed_percent: f32,
    pub settle_time_ms: u32,
}

#[derive(Debug)]
pub struct ProfileError(pub String);
impl fmt::Display for ProfileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
impl Error for ProfileError {}
impl From<serde_json::Error> for ProfileError {
    fn from(error: serde_json::Error) -> Self {
        Self(error.to_string())
    }
}

pub(crate) fn valid_name(name: &str, max: usize) -> bool {
    !name.is_empty()
        && name.len() <= max
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

pub fn validate_storage_name(name: &str) -> Result<(), ProfileError> {
    if valid_name(name, 26) && !matches!(name, "active_profile" | "storage_probe") {
        Ok(())
    } else {
        Err(ProfileError(
            "Invalid or reserved profile storage name".into(),
        ))
    }
}

impl Profile {
    pub fn from_json(bytes: &[u8]) -> Result<Self, ProfileError> {
        let value: serde_json::Value = serde_json::from_slice(bytes)?;
        let modern = [
            "fineStartPercent",
            "trickleStartPercent",
            "stopPercent",
            "coarseSpeedPercent",
            "fineSpeedPercent",
            "trickleSpeedPercent",
            "settleTimeMs",
        ];
        let profile = if modern.iter().any(|key| value.get(key).is_some()) {
            serde_json::from_value(value)?
        } else {
            #[derive(Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct Legacy {
                version: u8,
                id: String,
                name: String,
                coarse_speed: u8,
                fine_speed: u8,
                trickle_speed: u8,
            }
            let old: Legacy = serde_json::from_value(value)?;
            Self {
                version: old.version,
                id: old.id,
                name: old.name,
                fine_start_percent: 75.0,
                trickle_start_percent: 97.0,
                stop_percent: 99.5,
                coarse_speed_percent: old.coarse_speed as f32,
                fine_speed_percent: old.fine_speed as f32,
                trickle_speed_percent: old.trickle_speed as f32,
                settle_time_ms: 300,
            }
        };
        profile.validate()?;
        Ok(profile)
    }

    pub fn validate(&self) -> Result<(), ProfileError> {
        let percentages = [
            self.fine_start_percent,
            self.trickle_start_percent,
            self.stop_percent,
            self.coarse_speed_percent,
            self.fine_speed_percent,
            self.trickle_speed_percent,
        ];
        if self.version != 1
            || !valid_name(&self.id, 24)
            || self.name.is_empty()
            || self.name.len() > 64
            || !percentages
                .iter()
                .all(|p| p.is_finite() && (0.0..=100.0).contains(p))
            || self.fine_start_percent > self.trickle_start_percent
            || self.trickle_start_percent > self.stop_percent
        {
            return Err(ProfileError(
                "Invalid version, name, percentage, or threshold order".into(),
            ));
        }
        Ok(())
    }

    pub fn to_json(&self) -> Result<Vec<u8>, ProfileError> {
        self.validate()?;
        Ok(serde_json::to_vec(self)?)
    }
}
