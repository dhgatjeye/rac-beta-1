use std::ptr::null_mut;
use winapi::shared::windef::HWND;

pub struct Handle {
    handle: HWND,
}

unsafe impl Send for Handle {}
unsafe impl Sync for Handle {}

impl Handle {
    pub fn new() -> Self {
        Self { handle: null_mut() }
    }

    pub fn get(&self) -> HWND {
        self.handle
    }

    pub fn set(&mut self, handle: HWND) {
        self.handle = handle;
    }
}