use crate::config::constants::defaults;
use std::fs;
use std::io::{self};
use std::path::PathBuf;
use std::thread;
use std::time::Duration;

pub struct LogCleaner {
    max_size_bytes: usize,
    check_interval_secs: u64,
    running: bool,
}

impl LogCleaner {
    pub fn new(max_size_bytes: usize, check_interval_secs: u64) -> Self {
        Self {
            max_size_bytes,
            check_interval_secs,
            running: false,
        }
    }

    pub fn start(&mut self) {
        if self.running {
            return;
        }

        self.running = true;

        let max_size = self.max_size_bytes;
        let interval = self.check_interval_secs;

        thread::spawn(move || {
            loop {
                if let Err(e) = Self::clean_logs(max_size) {
                    eprintln!("Error cleaning logs: {}", e);
                }

                thread::sleep(Duration::from_secs(interval));
            }
        });
    }

    fn clean_logs(max_size: usize) -> io::Result<()> {
        let log_path = Self::get_log_file_path()?;

        if !log_path.exists() {
            return Ok(());
        }

        let metadata = fs::metadata(&log_path)?;

        if metadata.len() > max_size as u64 {
            fs::write(&log_path, "--- Log file cleaned due to size limit ---\n")?;
        }

        Ok(())
    }

    fn get_log_file_path() -> io::Result<PathBuf> {
        let local_app_data = dirs::data_local_dir()
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "Could not find AppData/Local directory"))?;

        let log_path = local_app_data.join(defaults::RAC_DIR).join(defaults::RAC_LOG_PATH);
        Ok(log_path)
    }
}