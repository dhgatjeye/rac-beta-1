use crate::config::constants::defaults;
use crate::logger::logger::{log_error, log_info};
use serde::{Deserialize, Serialize};
use std::io;
use std::path::PathBuf;

#[derive(Default, Serialize, Deserialize, Clone)]
pub struct Settings {
    pub target_process: String,
    pub toggle_key: i32,
    pub chest_collector_key: i32,
    pub adaptive_cpu_mode: bool,
    pub hotkey_hold_mode: bool,

    pub click_mode: String,
    pub left_max_cps: u8,
    pub right_max_cps: u8,

    pub left_game_mode: String,
    pub right_game_mode: String,
    pub post_mode: String,
    pub burst_mode: bool,
}

impl Settings {
    pub fn default_with_toggle_key(toggle_key: i32) -> Self {
        Self {
            target_process: defaults::TARGET_PROCESS.to_string(),
            toggle_key,
            chest_collector_key: defaults::CHEST_COLLECTOR_KEY,
            adaptive_cpu_mode: defaults::ADAPTIVE_CPU_MODE,
            hotkey_hold_mode: defaults::HOTKEY_HOLD_MODE,
            click_mode: defaults::CLICK_MODE.to_string(),
            left_max_cps: defaults::LEFT_MAX_CPS,
            right_max_cps: defaults::RIGHT_MAX_CPS,
            left_game_mode: defaults::LEFT_GAME_MODE.to_string(),
            right_game_mode: defaults::RIGHT_GAME_MODE.to_string(),
            post_mode: defaults::POST_MODE.to_string(),
            burst_mode: defaults::BURST_MODE,
        }
    }

    pub fn default() -> Self {
        Self::default_with_toggle_key(defaults::TOGGLE_KEY)
    }

    fn get_settings_path() -> io::Result<PathBuf> {
        let local_app_data = dirs::data_local_dir()
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "Could not find AppData/Local directory"))?;

        let settings_dir = local_app_data.join(defaults::RAC_DIR);
        if !settings_dir.exists() {
            std::fs::create_dir_all(&settings_dir)?;
        }

        Ok(settings_dir.join("settings.json"))
    }

    pub fn save(&self) -> io::Result<()> {
        let context = "Settings::save";
        match Self::get_settings_path() {
            Ok(settings_path) => {
                match serde_json::to_string_pretty(self) {
                    Ok(json) => {
                        if let Err(e) = std::fs::write(&settings_path, json) {
                            log_error(&format!("Failed to write settings file: {}", e), context);
                            return Err(e);
                        }
                        Ok(())
                    }
                    Err(e) => {
                        log_error(&format!("Failed to serialize settings: {}", e), context);
                        Err(io::Error::new(io::ErrorKind::Other, e))
                    }
                }
            }
            Err(e) => {
                log_error(&format!("Failed to get settings path: {}", e), context);
                Err(e)
            }
        }
    }

    pub fn load() -> io::Result<Self> {
        let context = "Settings::load";
        match Self::get_settings_path() {
            Ok(settings_path) => {
                if !settings_path.exists() {
                    let default_settings = Settings::default();
                    return Ok(default_settings);
                }

                let json = match std::fs::read_to_string(&settings_path) {
                    Ok(json) => json,
                    Err(e) => {
                        log_error(&format!("Failed to read settings file: {}", e), context);
                        return Err(e);
                    }
                };

                match serde_json::from_str(&json) {
                    Ok(settings) => Ok(settings),
                    Err(e) => {
                        log_error(&format!("Failed to parse settings JSON: {}", e), context);
                        log_info("Attempting to recover with partial settings", context);

                        let default_settings = Settings::default();
                        let partial: serde_json::Value = serde_json::from_str(&json).unwrap_or(serde_json::Value::Null);
                        let recovered_settings = serde_json::from_value(partial).unwrap_or(default_settings);

                        log_info("Recovered partial settings, using defaults for missing fields", context);
                        Ok(recovered_settings)
                    }
                }
            }
            Err(e) => {
                log_error(&format!("Failed to get settings path: {}", e), context);
                Err(e)
            }
        }
    }
}