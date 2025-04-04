use std::sync::{Arc, Mutex};
use winapi::um::winuser::{GetClientRect, GetForegroundWindow};
use windows::Win32::Graphics::Gdi::{GetDC, GetPixel, ReleaseDC};
use crate::input::handle::Handle;
use crate::input::window_finder::WindowFinder;

pub struct ChestDetector {
    chest_ui_color: u32,
    window_finder: WindowFinder,
    hwnd_handle: Arc<Mutex<Handle>>,
}

impl ChestDetector {
    pub fn new(target_process: &str) -> Self {
        Self {
            chest_ui_color: 0xC6C6C6,
            window_finder: WindowFinder::new(target_process),
            hwnd_handle: Arc::new(Mutex::new(Handle::new())),
        }
    }

    fn check_pixel_color(&self, x: i32, y: i32) -> bool {
        unsafe {
            let hdc = GetDC(None);
            let color = GetPixel(hdc, x, y);
            ReleaseDC(None, hdc);
            (color.0 & 0xFFFFFF) == self.chest_ui_color
        }
    }

    pub fn is_chest_open(&self) -> bool {
        if !self.is_window_focused() {
            return false;
        }

        if let Some(window) = self.window_finder.find_target_window(&self.hwnd_handle) {
            let mut rect = winapi::shared::windef::RECT::default();
            unsafe { GetClientRect(window, &mut rect) };

            let window_width = rect.right - rect.left;
            let window_height = rect.bottom - rect.top;

            let check_points = [
                (window_width / 2, window_height / 2 - 80),
                (window_width / 2 - 80, window_height / 2),
                (window_width / 2 + 80, window_height / 2),
            ];

            let mut match_count = 0;
            for (x, y) in check_points.iter() {
                if self.check_pixel_color(*x, *y) {
                    match_count += 1;
                }
            }

            match_count >= 2
        } else {
            false
        }
    }

    fn is_window_focused(&self) -> bool {
        if let Some(target_hwnd) = self.window_finder.find_target_window(&self.hwnd_handle) {
            unsafe {
                let foreground_hwnd = GetForegroundWindow();
                return foreground_hwnd == target_hwnd;
            }
        }
        false
    }
}
