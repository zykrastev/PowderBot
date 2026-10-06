use crate::{scale::Scale, stepper::Stepper};
use esp_idf_svc::hal::uart::UartDriver;
use powderbot_core::{controller::State, dashboard::Dashboard, profile_store::StoredProfile};
use std::{
    sync::{
        atomic::{AtomicBool, AtomicU32, Ordering},
        mpsc::{self, Receiver, SyncSender},
        Arc, Mutex,
    },
    thread,
    time::{Duration, Instant},
};

pub type Result<T> = std::result::Result<T, (u16, String)>;
pub enum Command {
    Start(Option<StoredProfile>),
    Stop,
    Tare,
    Reset,
    Settings(Vec<u8>),
    Profile(Option<StoredProfile>),
    ClearLoads,
}
struct Envelope {
    command: Command,
    deadline: Instant,
    epoch: u32,
    reply: SyncSender<Result<()>>,
}
#[derive(Clone, Debug)]
pub enum Event {
    State(State),
    ScaleError(String),
}
#[derive(Clone)]
pub struct Control {
    sender: SyncSender<Envelope>,
    pub gate: Arc<Mutex<()>>,
    published: Arc<Mutex<Dashboard>>,
    running: Arc<AtomicBool>,
    pending: Arc<AtomicBool>,
    stop_epoch: Arc<AtomicU32>,
    alive: Arc<AtomicBool>,
}
pub struct Worker {
    shutdown: Arc<AtomicBool>,
    thread: Option<thread::JoinHandle<()>>,
}
impl Drop for Worker {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::SeqCst);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
impl Control {
    pub fn busy(&self) -> bool {
        self.running.load(Ordering::SeqCst) || self.pending.load(Ordering::SeqCst)
    }
    pub fn snapshot(&self) -> Result<Dashboard> {
        if !self.alive.load(Ordering::SeqCst) {
            return Err((503, "Controller unavailable".into()));
        }
        self.published
            .lock()
            .map(|s| s.clone())
            .map_err(|_| (503, "Controller snapshot unavailable".into()))
    }
    /// Caller holds gate for starts, settings and profile changes. Stop bypasses it.
    pub fn submit(&self, command: Command) -> Result<()> {
        if !self.alive.load(Ordering::SeqCst) {
            return Err((503, "Controller unavailable".into()));
        }
        let starts = matches!(&command, Command::Start(_));
        if starts {
            if self.busy() {
                return Err((409, "Already dispensing".into()));
            }
            self.pending.store(true, Ordering::SeqCst);
        }
        if matches!(&command, Command::Stop | Command::Tare | Command::Reset) {
            self.stop_epoch.fetch_add(1, Ordering::SeqCst);
        }
        let epoch = self.stop_epoch.load(Ordering::SeqCst);
        let (reply, receive) = mpsc::sync_channel(1);
        let envelope = Envelope {
            command,
            deadline: Instant::now() + Duration::from_millis(750),
            epoch,
            reply,
        };
        if self.sender.try_send(envelope).is_err() {
            if starts {
                self.pending.store(false, Ordering::SeqCst);
            }
            return Err((503, "Controller command queue unavailable".into()));
        }
        match receive.recv_timeout(Duration::from_secs(1)) {
            Ok(result) => result,
            Err(_) => {
                // A timed-out start cannot leave an unobserved run active.
                if starts {
                    self.stop_epoch.fetch_add(1, Ordering::SeqCst);
                }
                Err((
                    504,
                    "Controller command timed out; check status before retrying".into(),
                ))
            }
        }
    }
}

pub fn spawn(
    mut motor: Stepper,
    uart: UartDriver<'static>,
    mut state: Dashboard,
) -> std::io::Result<(Control, Receiver<Event>, Worker)> {
    let (sender, receiver) = mpsc::sync_channel::<Envelope>(4);
    let (events, event_receiver) = mpsc::sync_channel(32);
    let shutdown = Arc::new(AtomicBool::new(false));
    let control = Control {
        sender,
        gate: Arc::new(Mutex::new(())),
        published: Arc::new(Mutex::new(state.clone())),
        running: Arc::new(AtomicBool::new(false)),
        pending: Arc::new(AtomicBool::new(false)),
        stop_epoch: Arc::new(AtomicU32::new(0)),
        alive: Arc::new(AtomicBool::new(true)),
    };
    let shared = control.clone();
    let quitting = shutdown.clone();
    let thread = thread::Builder::new()
        .name("controller".into())
        .stack_size(32768)
        .spawn(move || {
            let mut scale = Scale::new(uart);
            let mut epoch = 0;
            let mut previous = State::Idle;
            let mut scale_failed = false;
            while !quitting.load(Ordering::SeqCst) {
                let now = Instant::now();
                let latest = shared.stop_epoch.load(Ordering::SeqCst);
                if latest != epoch {
                    state.controller.stop();
                    epoch = latest;
                }
                state.tick(now);
                let early_result = apply_motor(&mut motor, &mut state);
                if let Err(error) = &early_result {
                    state.controller.fail(error.clone());
                    let _ = motor.stop();
                }
                if let Some(reading) = scale.update(now) {
                    if let Err(error) = &reading {
                        if !scale_failed {
                            let _ = events.try_send(Event::ScaleError(error.clone()));
                        }
                        scale_failed = true;
                    } else {
                        scale_failed = false;
                    }
                    state.record(reading, Instant::now());
                }
                state.tick(Instant::now());
                let mut completed = None;
                if let Ok(envelope) = receiver.try_recv() {
                    let starts = matches!(&envelope.command, Command::Start(_));
                    let result = if Instant::now() > envelope.deadline
                        || envelope.epoch != shared.stop_epoch.load(Ordering::SeqCst)
                    {
                        Err((409, "Command expired or cancelled by Stop".into()))
                    } else if let Err(error) = &early_result {
                        Err((500, error.clone()))
                    } else {
                        execute(envelope.command, &mut state, &mut scale, Instant::now())
                    };
                    completed = Some((envelope.reply, result, starts));
                }
                // Recheck priority stop after command parsing, before enabling the motor.
                let latest = shared.stop_epoch.load(Ordering::SeqCst);
                if latest != epoch {
                    state.controller.stop();
                    epoch = latest;
                    if let Some((_, result, true)) = &mut completed {
                        *result = Err((409, "Start cancelled by Stop".into()));
                    }
                }
                let mut motor_result = apply_motor(&mut motor, &mut state);
                if let Err(error) = &motor_result {
                    state.controller.fail(error.clone());
                    let _ = motor.stop();
                }
                // Stop may arrive during the driver's direction/enable setup waits.
                let latest = shared.stop_epoch.load(Ordering::SeqCst);
                if latest != epoch {
                    epoch = latest;
                    state.controller.stop();
                    motor_result = motor.stop().map_err(|e| e.to_string());
                    if let Err(error) = &motor_result {
                        state.controller.fail(error.clone());
                    }
                    if let Some((_, result, true)) = &mut completed {
                        *result = Err((409, "Start cancelled by Stop".into()));
                    }
                }
                shared
                    .running
                    .store(state.controller.state().active(), Ordering::SeqCst);
                let mut started = false;
                if let Some((reply, result, starts)) = completed {
                    if starts {
                        shared.pending.store(false, Ordering::SeqCst);
                    }
                    let result = motor_result.map_err(|e| (500, e)).and(result);
                    started = starts && result.is_ok();
                    let _ = reply.try_send(result);
                }
                let current = state.controller.state();
                if current != previous
                    || (started && matches!(current, State::Finished | State::Overthrown))
                {
                    let _ = events.try_send(Event::State(current));
                    previous = current;
                }
                if let Ok(mut snapshot) = shared.published.try_lock() {
                    *snapshot = state.clone();
                }
                // ESP-IDF's usleep busy-waits below one RTOS tick (10 ms here).
                // Always block this task so the idle task can feed its watchdog.
                esp_idf_svc::hal::delay::FreeRtos::delay_ms(10);
            }
            let _ = motor.stop();
            shared.alive.store(false, Ordering::SeqCst);
            shared.running.store(false, Ordering::SeqCst);
        })?;
    Ok((
        control,
        event_receiver,
        Worker {
            shutdown,
            thread: Some(thread),
        },
    ))
}
fn apply_motor(motor: &mut Stepper, state: &mut Dashboard) -> std::result::Result<(), String> {
    let speed = state.controller.speed();
    if speed == 0.0 {
        if motor.is_running() {
            motor.stop().map_err(|e| e.to_string())?;
        }
    } else {
        let hz = powderbot_core::motor::percent_to_hz(speed)
            .map_err(|_| "Invalid motor speed".to_owned())?;
        if motor.speed_hz() != hz {
            motor.set_speed_hz(hz).map_err(|e| e.to_string())?;
        }
        if !motor.is_running() {
            motor.start().map_err(|e| e.to_string())?;
            state.controller.motion_started(Instant::now());
        }
    }
    Ok(())
}
fn execute(command: Command, state: &mut Dashboard, scale: &mut Scale, now: Instant) -> Result<()> {
    match command {
        Command::Start(profile) => {
            if state.controller.state().active() {
                return Err((409, "Already dispensing".into()));
            }
            state.select(profile).map_err(|e| (409, e))?;
            state.start(now).map_err(|e| (409, e))
        }
        Command::Stop => {
            state.controller.stop();
            Ok(())
        }
        Command::Tare | Command::Reset => {
            state.controller.stop();
            state.invalidate();
            scale.tare(now).map_err(|e| {
                state.controller.fail(e.clone());
                (500, e)
            })
        }
        Command::Settings(bytes) => {
            if state.controller.state().active() {
                return Err((409, "Stop dispensing before changing settings".into()));
            }
            state.update_settings(&bytes).map_err(|e| (400, e))
        }
        Command::Profile(profile) => state.select(profile).map_err(|e| (409, e)),
        Command::ClearLoads => state.clear_load_history().map_err(|e| (409, e)),
    }
}
