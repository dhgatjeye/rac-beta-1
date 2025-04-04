use crate::input::handle::Handle;
use crate::input::window_finder::WindowFinder;
use rand::Rng;
use std::sync::{Arc, Mutex};
use std::{mem, thread};
use std::ffi::c_int;
use std::time::Duration;
use winapi::um::winuser::{GetForegroundWindow, PostMessageA, SendInput, INPUT, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, WM_KEYDOWN, WM_KEYUP, WM_LBUTTONDOWN, WM_LBUTTONUP};
use windows::Win32::Foundation::{GetLastError, POINT, WIN32_ERROR};
use windows::Win32::UI::Input::KeyboardAndMouse::VK_SHIFT;
use windows::Win32::UI::WindowsAndMessaging::{GetCursorPos, SetCursorPos};
use crate::supporter::chest_detector::ChestDetector;

const GRID_WIDTH: i32 = 9;
const GRID_HEIGHT: i32 = 3;

pub struct ChestCollector {
    is_active: bool,
    base_x: i32,
    base_y: i32,
    slot_size: i32,
    cached_positions: Vec<(i32, i32)>,
    window_finder: WindowFinder,
    hwnd_handle: Arc<Mutex<Handle>>,
    detector: ChestDetector,
}

impl ChestCollector {
    pub fn new(base_x: i32, base_y: i32, slot_size: i32, target_process: &str) -> Self {
        let mut cached_positions = Vec::with_capacity((GRID_WIDTH * GRID_HEIGHT) as usize);
        for row in 0..GRID_HEIGHT {
            for col in 0..GRID_WIDTH {
                cached_positions.push((
                    base_x + col * slot_size,
                    base_y + row * slot_size
                ));
            }
        }

        Self {
            is_active: false,
            base_x,
            base_y,
            slot_size,
            cached_positions,
            window_finder: WindowFinder::new(target_process),
            hwnd_handle: Arc::new(Mutex::new(Handle::new())),
            detector: ChestDetector::new(target_process),
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

    fn press_shift(&self) -> Result<(), WIN32_ERROR> {
        if !self.is_window_focused() {
            return Ok(());
        }
        Self::send_key_input(VK_SHIFT.0, false)
    }

    fn release_shift(&self) -> Result<(), WIN32_ERROR> {
        if !self.is_window_focused() {
            return Ok(());
        }
        Self::send_key_input(VK_SHIFT.0, true)
    }

    fn send_key_input(vk: u16, key_up: bool) -> Result<(), WIN32_ERROR> {
        let mut input = INPUT {
            type_: INPUT_KEYBOARD,
            u: unsafe { mem::zeroed() },
        };

        let kb_input = KEYBDINPUT {
            wVk: vk,
            wScan: 0,
            dwFlags: if key_up { KEYEVENTF_KEYUP } else { 0 },
            time: 0,
            dwExtraInfo: 0,
        };

        unsafe {
            *input.u.ki_mut() = kb_input;
            let result = SendInput(1, &mut input, size_of::<INPUT>() as c_int);
            if result == 0 {
                return Err(GetLastError());
            }
        }
        Ok(())
    }

    fn move_cursor(&self, target_x: i32, target_y: i32) -> Result<(), WIN32_ERROR> {
        if !self.is_window_focused() {
            return Ok(());
        }

        unsafe {
            let mut current = POINT { x: 0, y: 0 };
            if !GetCursorPos(&mut current).is_ok() {
                return Err(GetLastError());
            }

            let dx = target_x - current.x;
            let dy = target_y - current.y;
            let distance = ((dx * dx + dy * dy) as f32).sqrt();
            let steps = (distance / 20.0).clamp(5f32, 10f32) as i32;
            let sleep_per_step = 25 / steps as u64;

            let mut rng = rand::thread_rng();

            for i in 1..=steps {
                let t = i as f32 / steps as f32;
                let progress = t * t * (3.0 - 2.0 * t);

                let x_noise = (rng.gen::<f32>() - 0.5) * 1.0;
                let y_noise = (rng.gen::<f32>() - 0.5) * 1.0;

                let x = current.x + (dx as f32 * progress) as i32 + x_noise as i32;
                let y = current.y + (dy as f32 * progress) as i32 + y_noise as i32;

                if !SetCursorPos(x, y).is_ok() {
                    return Err(GetLastError());
                }
                thread::sleep(Duration::from_millis(sleep_per_step));
            }

            if !SetCursorPos(target_x, target_y).is_ok() {
                return Err(GetLastError());
            }

            Ok(())
        }
    }


    fn click(&self, x: i32, y: i32) -> Result<(), WIN32_ERROR> {
        if !self.is_window_focused() {
            return Ok(());
        }

        let hwnd = self.hwnd_handle.lock().unwrap().get();

        self.move_cursor(x, y)?;

        let flags = 1 | (1 << 16);
        unsafe {
            PostMessageA(hwnd, WM_LBUTTONDOWN, flags, 0);
            PostMessageA(hwnd, WM_LBUTTONUP, 0, 0);
        }

        Ok(())
    }

    pub fn collect_items(&self) {
        let mut rng = rand::thread_rng();
        let mut uncollected_slots: Vec<usize> = (0..(GRID_WIDTH * GRID_HEIGHT) as usize).collect();

        while !uncollected_slots.is_empty() {
            let batch_size = 3.min(uncollected_slots.len());
            for _ in 0..batch_size {
                if uncollected_slots.is_empty() { break; }

                let slot_index = rng.random_range(0..uncollected_slots.len());
                let slot = uncollected_slots.remove(slot_index);
                let (base_x, base_y) = self.cached_positions[slot];

                let x = base_x + rng.random_range(-1..=1);
                let y = base_y + rng.random_range(-1..=1);

                let _ = self.click(x, y);
            }

            thread::sleep(Duration::from_millis(5));
        }
    }

    pub fn toggle(&mut self) -> Result<(), WIN32_ERROR> {
        if self.detector.is_chest_open() {
            self.is_active = !self.is_active;
            if self.is_active {
                Ok(self.collect_items())
            } else {
                Ok(())
            }
        } else {
            self.is_active = false;
            Ok(())
        }
    }

    pub fn is_active(&self) -> bool {
        self.is_active
    }

    pub fn update_target_process(&self, new_target_process: &str) -> bool {
        self.window_finder.update_target_process(new_target_process)
    }
}