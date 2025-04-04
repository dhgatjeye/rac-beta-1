use crate::helper::log_cleaner::LogCleaner;
use crate::input::click_executor::ClickExecutor;
pub use crate::input::click_service::{ClickService, ClickServiceConfig};
pub use crate::menu::Menu;
use crate::validation::system_validator::SystemValidator;
use std::sync::Arc;
use windows::core::w;
use windows::Win32::Foundation::{GetLastError, ERROR_ALREADY_EXISTS};
use windows::Win32::System::Threading::CreateMutexW;

pub mod config;
pub mod input;
pub mod menu;
pub mod validation;
mod logger;
mod auth;
mod helper;
mod supporter;

pub struct ClickServiceMenu {
    pub click_service: Arc<ClickService>,
    pub click_executor: Arc<ClickExecutor>,
}

impl ClickServiceMenu {
    pub fn new(click_service: Arc<ClickService>, click_executor: Arc<ClickExecutor>) -> Self {
        Self {
            click_service,
            click_executor,
        }
    }
}

pub fn initialize_services() -> Result<(), String> {
    let validator = SystemValidator::new();
    let validation_result = validator.validate_system();
    if !validation_result.is_valid {
        return Err(validation_result.message.unwrap_or_else(|| "Unknown validation error".to_string()));
    }

    let mut log_cleaner = LogCleaner::new(1_000_000, 60);
    log_cleaner.start();

    Ok(())
}

pub fn check_single_instance() -> bool {
    unsafe {
        let mutex_name = w!("Global\\RACApplicationMutex");
        CreateMutexW(None, true, mutex_name).expect("TODO: panic message");
        GetLastError() != ERROR_ALREADY_EXISTS
    }
}