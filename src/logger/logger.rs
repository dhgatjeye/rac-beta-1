use crate::config::constants::defaults;
use chrono::Utc;
use lazy_static::lazy_static;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;

#[derive(Debug)]
pub enum LogLevel {
    Info,
    Warning,
    Error,
}

impl LogLevel {
    fn as_str(&self) -> &'static str {
        match self {
            LogLevel::Info => "INFO",
            LogLevel::Warning => "WARN",
            LogLevel::Error => "ERROR"
        }
    }
}

lazy_static! {
    static ref LOGGER: Mutex<Logger> = Mutex::new(Logger::new());
}

pub struct Logger {
    log_file: PathBuf,
}

impl Logger {
    fn new() -> Self {
        let log_path = dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(defaults::RAC_DIR)
            .join(defaults::RAC_LOG_PATH);

        if let Some(parent) = log_path.parent() {
            fs::create_dir_all(parent).unwrap_or_else(|e| {
                eprintln!("Failed to create log directory: {}", e);
            });
        }

        Self { log_file: log_path }
    }

    fn write_log(&self, level: LogLevel, message: &str, context: &str) {
        if let Ok(mut file) = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.log_file)
        {
            let timestamp = Utc::now().format("%Y-%m-%d %H:%M:%S");
            let log_entry = format!(
                "[{}] [{}] {} in {}\n{}\n{}\n",
                timestamp,
                level.as_str(),
                message,
                context,
                "-".repeat(80),
                ""
            );

            if let Err(e) = file.write_all(log_entry.as_bytes()) {
                eprintln!("Failed to write log: {}", e);
            }
        }
    }
}

pub fn log_error(error: &str, context: &str) {
    if let Ok(logger) = LOGGER.lock() {
        logger.write_log(LogLevel::Error, error, context);
    }
}

pub fn log_info(message: &str, context: &str) {
    if let Ok(logger) = LOGGER.lock() {
        logger.write_log(LogLevel::Info, message, context);
    }
}

pub fn log_warn(message: &str, context: &str) {
    if let Ok(logger) = LOGGER.lock() {
        logger.write_log(LogLevel::Warning, message, context);
    }
}