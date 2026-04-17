use std::path::PathBuf;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

pub struct LogConfig;

impl LogConfig {
    pub fn init() -> anyhow::Result<(PathBuf, WorkerGuard)> {
        let log_dir = dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("vineoffice")
            .join("logs");

        std::fs::create_dir_all(&log_dir)?;

        let timestamp = chrono::Local::now().format("%Y-%m-%d_%H-%M-%S");
        let log_file = log_dir.join(format!("install_{}.log", timestamp));

        let file_appender =
            tracing_appender::rolling::never(&log_dir, format!("install_{}.log", timestamp));
        let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

        let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

        tracing_subscriber::registry()
            .with(
                fmt::layer()
                    .with_writer(non_blocking)
                    .with_ansi(false) // Disable ANSI escape codes for file output
            )
            .with(filter)
            .init();

        Ok((log_file, guard))
    }
}

#[derive(Clone, Debug)]
pub struct LogEntry {
    pub timestamp: String,
    pub level: LogLevel,
    pub message: String,
}

#[derive(Clone, Debug, PartialEq)]
pub enum LogLevel {
    Info,
    Warn,
    #[allow(dead_code)]
    Error,
}

impl LogEntry {
    pub fn new(level: LogLevel, message: impl Into<String>) -> Self {
        Self {
            timestamp: chrono::Local::now().format("%H:%M:%S").to_string(),
            level,
            message: message.into(),
        }
    }
}
