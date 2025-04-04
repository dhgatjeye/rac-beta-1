use crate::auth::license_validator::LicenseValidator;
use crate::logger::logger::{log_error, log_info};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::net::UdpSocket;
use tokio::time;

pub struct LicenseChecker {
    validator: Arc<LicenseValidator>,
    is_running: Arc<AtomicBool>,
}

impl LicenseChecker {
    pub fn new(validator: LicenseValidator) -> Self {
        Self {
            validator: Arc::new(validator),
            is_running: Arc::new(AtomicBool::new(true)),
        }
    }

    pub async fn detect_time_manipulation() -> bool {
        use std::cmp::{max, min};

        let system_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let network_time = Self::fetch_network_time().await;
        let difference = max(system_time, network_time) - min(system_time, network_time);

        if difference >= 60 {
            log_error(&format!("Time manipulation detected: {}s difference", difference), "LicenseChecker::detect_time_manipulation");
            return false;
        }
        true
    }

    pub async fn fetch_network_time() -> u64 {
        let ntp_servers = [
            "pool.ntp.org",
            "time.google.com",
            "time.windows.com",
            "time.apple.com"
        ];

        for server in ntp_servers {
            if let Ok(time) = Self::fetch_time_from_server(server).await {
                return time;
            }
        }

        let fallback_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        log_error("Failed to fetch network time from all servers", "LicenseChecker::fetch_network_time");
        fallback_time
    }

    async fn fetch_time_from_server(server: &str) -> Result<u64, Box<dyn std::error::Error>> {
        let socket = UdpSocket::bind("0.0.0.0:0").await?;
        socket.connect(format!("{}:123", server)).await?;

        let ntp_msg = [0x1B; 48];
        socket.send(&ntp_msg).await?;

        let mut buf = [0; 48];
        let timeout = time::timeout(Duration::from_secs(5), socket.recv(&mut buf)).await??;

        if timeout < 48 {
            return Err("Incomplete NTP response".into());
        }

        let ntp_seconds = u32::from_be_bytes([buf[40], buf[41], buf[42], buf[43]]);
        Ok((ntp_seconds as u64).saturating_sub(2208988800))
    }

    pub async fn start_checking(&self) {
        let validator = Arc::clone(&self.validator);
        let is_running = Arc::clone(&self.is_running);

        tokio::spawn(async move {
            let mut interval = time::interval(Duration::from_secs(150));

            loop {
                interval.tick().await;

                if !is_running.load(Ordering::SeqCst) {
                    break;
                }

                if !Self::detect_time_manipulation().await {
                    log_error("DTM detected - exiting", "LicenseChecker::start_checking");
                    std::process::exit(1);
                }

                match validator.validate_license() {
                    Ok(true) => {
                        log_info("License check passed", "LicenseChecker::start_checking");
                    }
                    Ok(false) => {
                        log_error("License has expired or is invalid", "LicenseChecker::start_checking");
                        std::process::exit(1);
                    }
                    Err(e) => {
                        log_error(&format!("License validation error: {}", e), "LicenseChecker::start_checking");
                        std::process::exit(1);
                    }
                }
            }
        });
    }
}