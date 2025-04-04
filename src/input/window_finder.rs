use crate::input::handle::Handle;
use std::ptr::null_mut;
use std::sync::{Arc, Mutex};
use sysinfo::{ProcessesToUpdate, System};
use winapi::{
    shared::{minwindef::{DWORD, LPARAM}, windef::HWND},
    um::winuser::{EnumWindows, GetWindowThreadProcessId, IsWindowVisible},
};

struct FindWindowData {
    pid: DWORD,
    hwnd: HWND,
    window_count: u32,
    require_visibility: bool,
}

unsafe extern "system" fn enum_windows_callback(hwnd: HWND, lparam: LPARAM) -> i32 {
    let data = &mut *(lparam as *mut FindWindowData);
    let mut process_id: DWORD = 0;
    GetWindowThreadProcessId(hwnd, &mut process_id);

    if process_id == data.pid {
        let is_visible = IsWindowVisible(hwnd) != 0;
        if !data.require_visibility || is_visible {
            data.hwnd = hwnd;
            data.window_count += 1;
            return 1;
        }
    }
    1
}

pub struct WindowFinder {
    target_process: String,
    system: Arc<Mutex<System>>,
    last_found_pid: Option<DWORD>,
    require_visibility: bool,
}

impl WindowFinder {
    pub fn new(target_process: &str) -> Self {
        Self {
            target_process: target_process.to_string(),
            system: Arc::new(Mutex::new(System::new_all())),
            last_found_pid: None,
            require_visibility: true,
        }
    }

    pub fn update_target_process(&self, new_target_process: &str) -> bool {
        if self.target_process == new_target_process {
            return false;
        }

        unsafe {
            let self_ptr = self as *const WindowFinder as *mut WindowFinder;
            (*self_ptr).target_process = new_target_process.to_string();
            (*self_ptr).last_found_pid = None;
        }

        true
    }

    pub fn find_target_window(&self, hwnd_handle: &Arc<Mutex<Handle>>) -> Option<HWND> {
        let mut reset_pid = false;
        if let Some(pid) = self.last_found_pid {
            if let Some(hwnd) = self.find_window_for_pid(pid) {
                let mut hwnd_guard = hwnd_handle.lock().unwrap();
                hwnd_guard.set(hwnd);
                return Some(hwnd);
            } else {
                reset_pid = true;
            }
        }

        let mut sys = self.system.lock().unwrap();
        sys.refresh_processes(ProcessesToUpdate::All, false);

        let mut target_pid: Option<DWORD> = None;
        for (pid, process) in sys.processes() {
            let name = process.name().to_string_lossy();
            if name.to_lowercase() == self.target_process.to_lowercase() {
                target_pid = Some(pid.as_u32());
                break;
            }
        }

        drop(sys);

        if let Some(pid) = target_pid {
            unsafe {
                let self_ptr = self as *const WindowFinder as *mut WindowFinder;
                (*self_ptr).last_found_pid = Some(pid);
            }

            if let Some(hwnd) = self.find_window_for_pid(pid) {
                let mut hwnd_guard = hwnd_handle.lock().unwrap();
                hwnd_guard.set(hwnd);
                return Some(hwnd);
            }
        } else {
            unsafe {
                let self_ptr = self as *const WindowFinder as *mut WindowFinder;
                (*self_ptr).last_found_pid = None;
            }
        }

        if reset_pid {
            unsafe {
                let self_ptr = self as *const WindowFinder as *mut WindowFinder;
                (*self_ptr).last_found_pid = None;
            }
        }

        let mut hwnd_guard = hwnd_handle.lock().unwrap();
        hwnd_guard.set(null_mut());
        None
    }

    fn find_window_for_pid(&self, pid: DWORD) -> Option<HWND> {
        let mut data = FindWindowData {
            pid,
            hwnd: null_mut(),
            window_count: 0,
            require_visibility: self.require_visibility,
        };

        unsafe {
            EnumWindows(Some(enum_windows_callback), &mut data as *mut _ as LPARAM);

            if !data.hwnd.is_null() {
                return Some(data.hwnd);
            }
        }

        None
    }
}