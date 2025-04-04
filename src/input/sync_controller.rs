use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Condvar, Mutex};
use std::time::Duration;

pub struct SyncController {
    enabled: AtomicBool,
    mutex: Mutex<bool>,
    condvar: Condvar,
}

impl SyncController {
    pub fn new() -> Self {
        Self {
            enabled: AtomicBool::new(false),
            mutex: Mutex::new(false),
            condvar: Condvar::new(),
        }
    }

    pub fn toggle(&self) -> bool {
        let new_state = !self.enabled.load(Ordering::SeqCst);
        self.enabled.store(new_state, Ordering::SeqCst);

        let mut enabled = self.mutex.lock().unwrap();
        *enabled = new_state;
        self.condvar.notify_all();

        new_state
    }

    pub fn force_enable(&self) -> bool {
        if self.enabled.load(Ordering::SeqCst) {
            return true;
        }

        self.enabled.store(true, Ordering::SeqCst);

        let mut enabled = self.mutex.lock().unwrap();
        *enabled = true;

        self.condvar.notify_all();

        true
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::SeqCst)
    }

    pub fn wait_for_signal(&self, timeout: Duration) -> bool {
        let mut enabled = self.mutex.lock().unwrap();

        let atomic_enabled = self.enabled.load(Ordering::SeqCst);

        if *enabled != atomic_enabled {
            *enabled = atomic_enabled;
        }

        if !*enabled {
            let result = self.condvar.wait_timeout(enabled, timeout).unwrap();
            enabled = result.0;

            if !*enabled && self.enabled.load(Ordering::SeqCst) {
                *enabled = true;
            }
        }

        *enabled
    }
}