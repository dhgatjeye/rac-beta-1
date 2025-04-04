use crate::logger::logger::log_error;
use std::thread;
use std::time::Duration;
use std::time::Instant;
use windows::Win32::System::Threading::{GetCurrentThread, SetThreadPriority};
use windows::Win32::System::Threading::{THREAD_PRIORITY_BELOW_NORMAL, THREAD_PRIORITY_NORMAL, THREAD_PRIORITY_TIME_CRITICAL};

pub struct ThreadController {
    adaptive_mode: bool,
}

impl ThreadController {
    pub(crate) fn clone(&self) -> ThreadController {
        ThreadController {
            adaptive_mode: self.adaptive_mode,
        }
    }
}

impl ThreadController {
    pub fn new(adaptive_mode: bool) -> Self {
        Self { adaptive_mode }
    }

    pub fn set_adaptive_mode(&self, adaptive_mode: bool) {
        unsafe {
            let self_ptr = self as *const ThreadController as *mut ThreadController;
            (*self_ptr).adaptive_mode = adaptive_mode;
        }
    }

    pub fn set_active_priority(&self) {
        let context = "ThreadController::set_active_priority";
        unsafe {
            let priority = if self.adaptive_mode {
                THREAD_PRIORITY_NORMAL
            } else {
                THREAD_PRIORITY_TIME_CRITICAL
            };

            if let Err(e) = SetThreadPriority(GetCurrentThread(), priority) {
                log_error(&format!("Failed to set active thread priority: {:?}", e), context);
            }
        }
    }

    pub fn set_normal_priority(&self) {
        let context = "ThreadController::set_normal_priority";
        unsafe {
            if let Err(e) = SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_NORMAL) {
                log_error(&format!("Failed to set normal thread priority: {:?}", e), context);
            }
        }
    }

    pub fn set_idle_priority(&self) {
        let context = "ThreadController::set_idle_priority";
        unsafe {
            if let Err(e) = SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_BELOW_NORMAL) {
                log_error(&format!("Failed to set idle thread priority: {:?}", e), context);
            }
        }
    }

    pub fn smart_sleep(&self, duration: Duration) {
        if duration.as_micros() < 1 {
            return;
        }

        if duration.as_micros() < 1000 {
            let start = Instant::now();
            while start.elapsed() < duration {}
            return;
        }

        thread::sleep(duration);
    }
}