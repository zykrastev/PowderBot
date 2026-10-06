
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Instant;

use esp_idf_svc::log::EspIdfLogger;
use log::{Level, LevelFilter, Log, Metadata, Record};
use powderbot_core::log_buffer::LogBuffer;

pub type SharedLogs = Arc<Mutex<LogBuffer>>;

struct MirrorLogger {
    serial: EspIdfLogger,
    history: SharedLogs,
    started: Instant,
}

static LOGGER: OnceLock<MirrorLogger> = OnceLock::new();

pub fn init() -> Result<SharedLogs, log::SetLoggerError> {
    let logger = LOGGER.get_or_init(|| MirrorLogger {
        serial: EspIdfLogger::new(()),
        history: Arc::new(Mutex::new(LogBuffer::default())),
        started: Instant::now(),
    });
    log::set_logger(logger)?;
    log::set_max_level(LevelFilter::Info);
    Ok(Arc::clone(&logger.history))
}

impl Log for MirrorLogger {
    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        metadata.level() <= Level::Info
    }

    fn log(&self, record: &Record<'_>) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let line = format!(
            "[{:>8} ms] {:5} {}: {}",
            self.started.elapsed().as_millis(),
            record.level(),
            record.target(),
            record.args()
        );
        {
            let mut history = self
                .history
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            history.push(&line);
        }
        self.serial.log(record);
    }

    fn flush(&self) {
        self.serial.flush();
    }
}
