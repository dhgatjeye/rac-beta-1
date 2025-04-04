use crate::config::settings::Settings;
use crate::input::click_executor::PostMode;
use crate::logger::logger::{log_error, log_info};
use rand::Rng;
use std::time::Duration;

pub struct DelayProvider {
    delay_buffer: Vec<Duration>,
    current_index: usize,
    pub(crate) burst_mode: bool,
    burst_counter: u8,
    pub(crate) post_mode: PostMode,
}

impl DelayProvider {
    pub fn new() -> Self {
        let context = "DelayProvider::new";

        let settings = Settings::load().unwrap_or_else(|_| Settings::default());

        let mut provider = Self {
            delay_buffer: vec![Duration::ZERO; 512],
            current_index: 0,
            burst_mode: settings.burst_mode,
            burst_counter: 0,
            post_mode: match settings.post_mode.as_str() {
                "Bedwars" => PostMode::Bedwars,
                _ => PostMode::Default,
            },
        };

        match provider.initialize_delay_buffer() {
            Ok(_) => {
                log_info("Delay buffer initialized successfully", context);
                provider
            }
            Err(e) => {
                log_error(&format!("Failed to initialize delay buffer: {}", e), context);
                provider
            }
        }
    }

    pub fn toggle_burst_mode(&mut self) -> bool {
        self.burst_mode = !self.burst_mode;
        self.burst_counter = 0;
        self.burst_mode
    }

    fn initialize_delay_buffer(&mut self) -> Result<(), String> {
        let mut rng = rand::rng();
        for delay in self.delay_buffer.iter_mut() {
            let ms = rng.random_range(2.0..=5.0);
            *delay = Duration::from_micros((ms * 1000.0) as u64);
        }
        Ok(())
    }

    pub fn get_next_delay(&mut self) -> Duration {
        let mut rng = rand::rng();

        if self.burst_mode && self.burst_counter < 1 {
            self.burst_counter += 1;
            return Duration::from_micros(rng.random_range(3000..4000));
        } else if self.burst_mode {
            self.burst_counter = 0;
        }

        let base_delay = self.delay_buffer[self.current_index];
        self.current_index = (self.current_index + 1) & 511;

        let micro_adjust: i32 = rng.random_range(-50..50);

        let final_delay = if micro_adjust < 0 {
            base_delay.saturating_sub(Duration::from_micros(-micro_adjust as u64))
        } else {
            base_delay.saturating_add(Duration::from_micros(micro_adjust as u64))
        };

        let settings = Settings::load().unwrap_or_else(|_| Settings::default());
        let post_mode = match settings.post_mode.as_str() {
            "Bedwars" => PostMode::Bedwars,
            _ => PostMode::Default,
        };

        let final_delay = match post_mode {
            PostMode::Bedwars => {
                final_delay
            }
            PostMode::Default => {
                if final_delay < Duration::from_micros(200) {
                    Duration::from_micros(200)
                } else {
                    final_delay
                }
            }
        };

        final_delay
    }
}