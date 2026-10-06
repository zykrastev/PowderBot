use powderbot_core::profile::Profile;

pub fn profile() -> Profile {
    Profile::from_json(br#"{"version":1,"id":"p1","name":"Test powder","fineStartPercent":75,"trickleStartPercent":97,"stopPercent":99.5,"coarseSpeedPercent":100,"fineSpeedPercent":30,"trickleSpeedPercent":50,"settleTimeMs":300}"#).unwrap()
}
