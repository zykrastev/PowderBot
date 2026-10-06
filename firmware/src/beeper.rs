
use std::sync::mpsc::{self, SyncSender, TrySendError};
use std::thread;
use std::time::Duration;

use esp_idf_svc::hal::ledc::{LedcDriver, LedcTimerDriver, LowSpeed};
use esp_idf_svc::hal::units::Hertz;
use esp_idf_svc::sys::EspError;

#[derive(Clone, Copy, Debug)]
pub enum Sound {
    Boot,
    Error,
}

// (frequency in Hz, duration in milliseconds), matching the C++ chimes.
const BOOT: &[(u32, u64)] = &[(523, 80), (659, 80), (784, 120)];
const ERROR: &[(u32, u64)] = &[(300, 500)];

pub struct Beeper {
    sender: SyncSender<Sound>,
}

impl Beeper {
    pub fn new(
        mut pwm: LedcDriver<'static>,
        mut timer: LedcTimerDriver<'static, LowSpeed>,
    ) -> std::io::Result<Self> {
        let (sender, receiver) = mpsc::sync_channel::<Sound>(4);
        thread::Builder::new()
            .name("beeper".into())
            .stack_size(4096)
            .spawn(move || {
                while let Ok(sound) = receiver.recv() {
                    let notes = match sound {
                        Sound::Boot => BOOT,
                        Sound::Error => ERROR,
                    };
                    if let Err(error) = play_notes(&mut pwm, &mut timer, notes) {
                        log::error!("Beeper playback failed: {error}");
                        break;
                    }
                }
                // Silence on channel closure or a playback error.
                if let Err(error) = pwm.set_duty(0) {
                    log::error!("Could not silence beeper: {error}");
                }
            })?;
        Ok(Self { sender })
    }

    pub fn play(&self, sound: Sound) -> Result<(), TrySendError<Sound>> {
        self.sender.try_send(sound)
    }
}

fn play_notes(
    pwm: &mut LedcDriver<'_>,
    timer: &mut LedcTimerDriver<'_, LowSpeed>,
    notes: &[(u32, u64)],
) -> Result<(), EspError> {
    for &(frequency, duration_ms) in notes {
        timer.set_frequency(Hertz(frequency))?;
        pwm.set_duty(pwm.get_max_duty() / 2)?;
        thread::sleep(Duration::from_millis(duration_ms));
        pwm.set_duty(0)?;
        thread::sleep(Duration::from_millis(20));
    }
    Ok(())
}
