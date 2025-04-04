use crate::config::constants::defaults;
use crate::config::settings::Settings;
use crate::input::click_executor::{ClickExecutor, GameMode, MouseButton};
use crate::input::delay_provider::DelayProvider;
use crate::input::handle::Handle;
use crate::input::sync_controller::SyncController;
use crate::input::thread_controller::ThreadController;
use crate::input::window_finder::WindowFinder;
use crate::logger::logger::{log_error, log_info};
use std::panic::AssertUnwindSafe;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use winapi::shared::windef::HWND;
use winapi::um::winuser::GetAsyncKeyState;

struct ButtonComponents {
    click_controller: Arc<SyncController>,
    delay_provider: Arc<Mutex<DelayProvider>>,
    thread_controller: Arc<ThreadController>,
    click_executor: Arc<ClickExecutor>,
}

struct SettingsChanges {
    target_process: bool,
    adaptive_cpu_mode: bool,
}

pub struct ClickServiceConfig {
    pub target_process: String,

    pub window_check_active_interval: Duration,
    pub window_check_idle_interval: Duration,

    pub adaptive_cpu_mode: bool,
}

impl Default for ClickServiceConfig {
    fn default() -> Self {
        let settings = Settings::load().unwrap_or_else(|_| Settings::default());

        Self {
            target_process: settings.target_process,

            window_check_active_interval: Duration::from_secs(1),
            window_check_idle_interval: Duration::from_secs(3),

            adaptive_cpu_mode: settings.adaptive_cpu_mode,
        }
    }
}

pub struct ClickService {
    config: ClickServiceConfig,
    settings: Arc<Mutex<Settings>>,

    hwnd: Arc<Mutex<Handle>>,
    window_finder: Arc<WindowFinder>,
    window_finder_running: Arc<AtomicBool>,

    sync_controller: Arc<SyncController>,
    pub click_executor: Arc<ClickExecutor>,
    pub(crate) delay_provider: Arc<Mutex<DelayProvider>>,

    left_click_controller: Arc<SyncController>,
    left_delay_provider: Arc<Mutex<DelayProvider>>,
    left_thread_controller: Arc<ThreadController>,
    pub(crate) left_click_executor: Arc<ClickExecutor>,

    right_click_controller: Arc<SyncController>,
    right_delay_provider: Arc<Mutex<DelayProvider>>,
    right_thread_controller: Arc<ThreadController>,
    pub(crate) right_click_executor: Arc<ClickExecutor>,
}

impl ClickService {
    pub fn new(config: ClickServiceConfig) -> Arc<Self> {
        let context = "ClickService::new";

        let settings = Settings::load().unwrap_or_else(|_| Settings::default());
        let settings_clone = settings.clone();
        let adaptive_cpu_mode = config.adaptive_cpu_mode;

        let target_process = config.target_process.clone();

        let left_thread_controller = Arc::new(ThreadController::new(adaptive_cpu_mode));
        let right_thread_controller = Arc::new(ThreadController::new(adaptive_cpu_mode));

        let service = Arc::new(Self {
            config,
            settings: Arc::new(Mutex::new(settings)),

            hwnd: Arc::new(Mutex::new(Handle::new())),
            window_finder: Arc::new(WindowFinder::new(&target_process)),
            window_finder_running: Arc::new(AtomicBool::new(true)),

            sync_controller: Arc::new(SyncController::new()),
            click_executor: Arc::new(ClickExecutor::new((*left_thread_controller).clone())),
            delay_provider: Arc::new(Mutex::new(DelayProvider::new())),

            left_click_controller: Arc::new(SyncController::new()),
            left_delay_provider: Arc::new(Mutex::new(DelayProvider::new())),
            left_thread_controller: left_thread_controller.clone(),
            left_click_executor: Arc::new(ClickExecutor::new((*left_thread_controller).clone())),

            right_click_controller: Arc::new(SyncController::new()),
            right_delay_provider: Arc::new(Mutex::new(DelayProvider::new())),
            right_thread_controller: right_thread_controller.clone(),
            right_click_executor: Arc::new(ClickExecutor::new((*right_thread_controller).clone())),
        });

        let left_click_executor = Arc::clone(&service.left_click_executor);
        left_click_executor.set_max_cps(settings_clone.left_max_cps);
        left_click_executor.set_mouse_button(MouseButton::Left);
        let left_mode = match settings_clone.left_game_mode.as_str() {
            "Combo" => GameMode::Combo,
            _ => GameMode::Default,
        };
        left_click_executor.set_game_mode(left_mode);

        let right_click_executor = Arc::clone(&service.right_click_executor);
        right_click_executor.set_max_cps(settings_clone.right_max_cps);
        right_click_executor.set_mouse_button(MouseButton::Right);
        let right_mode = match settings_clone.right_game_mode.as_str() {
            "Combo" => GameMode::Combo,
            _ => GameMode::Default,
        };
        right_click_executor.set_game_mode(right_mode);

        let service_clone = service.clone();
        match thread::Builder::new()
            .name("WindowFinderThread".to_string())
            .spawn(move || {
                service_clone.window_finder_loop();
            }) {
            Ok(_) => {
                log_info("Window finder thread spawned successfully", context);
            }
            Err(e) => {
                log_error(&format!("Failed to spawn window finder thread: {}", e), context);
            }
        }

        let service_clone = service.clone();
        match thread::Builder::new()
            .name("SettingsSyncThread".to_string())
            .spawn(move || {
                service_clone.settings_sync_loop();
            }) {
            Ok(_) => {
                log_info("Settings synchronization thread spawned successfully", context);
            }
            Err(e) => {
                log_error(&format!("Failed to spawn settings sync thread: {}", e), context);
            }
        }

        let service_clone = service.clone();
        Self::spawn_click_thread("LeftClickThread", service_clone.clone(), MouseButton::Left);

        let service_clone = service.clone();
        Self::spawn_click_thread("RightClickThread", service_clone.clone(), MouseButton::Right);

        service
    }

    pub fn start(&self) {
        self.left_click_executor.set_active(true);
        self.right_click_executor.set_active(true);
    }

    pub fn stop(&self) {
        self.left_click_executor.set_active(false);
        self.right_click_executor.set_active(false);
    }

    pub fn click_loop(&self, button: MouseButton) {
        let (context, components) = self.get_button_components(button);
        let ButtonComponents {
            click_controller,
            delay_provider,
            thread_controller,
            click_executor,
        } = components;

        thread_controller.set_active_priority();
        thread_controller.set_adaptive_mode(!self.config.adaptive_cpu_mode);

        let mut consecutive_failures = 0;
        let mut last_click = Instant::now();
        let mut last_button_state = false;

        self.configure_click_executor(button, &click_executor);

        while !thread::panicking() {
            if !click_controller.wait_for_signal(Duration::from_millis(50)) {
                continue;
            }

            let settings = Settings::load().unwrap_or_default();

            let should_click = if settings.hotkey_hold_mode {
                click_executor.is_active()
            } else {
                let is_pressed = self.is_mouse_button_pressed(button);

                if last_button_state && !is_pressed {
                    match button {
                        MouseButton::Left => self.left_click_executor.handle_button_release(),
                        MouseButton::Right => self.right_click_executor.handle_button_release(),
                    }
                }

                last_button_state = is_pressed;
                is_pressed && click_executor.is_active()
            };

            if !should_click {
                continue;
            }

            let hwnd = self.get_current_hwnd();

            if click_executor.execute_click(hwnd) {
                consecutive_failures = 0;

                let delay = self.get_next_delay(&delay_provider);
                let elapsed = last_click.elapsed();

                if elapsed < delay {
                    thread_controller.smart_sleep(delay.saturating_sub(elapsed));
                }

                last_click = Instant::now();
            } else {
                consecutive_failures = (consecutive_failures + 1) % 3;
                thread_controller.smart_sleep(Duration::from_millis(20));
            }
        }

        self.window_finder_running.store(false, Ordering::SeqCst);
        log_error("Click loop terminated due to thread panic", &context);
    }

    fn get_button_components(&self, button: MouseButton) -> (&str, ButtonComponents) {
        let context = match button {
            MouseButton::Left => "ClickService::left_click_loop",
            MouseButton::Right => "ClickService::right_click_loop",
        };

        let components = match button {
            MouseButton::Left => ButtonComponents {
                click_controller: Arc::clone(&self.left_click_controller),
                delay_provider: Arc::clone(&self.left_delay_provider),
                thread_controller: Arc::clone(&self.left_thread_controller),
                click_executor: Arc::clone(&self.left_click_executor),
            },
            MouseButton::Right => ButtonComponents {
                click_controller: Arc::clone(&self.right_click_controller),
                delay_provider: Arc::clone(&self.right_delay_provider),
                thread_controller: Arc::clone(&self.right_thread_controller),
                click_executor: Arc::clone(&self.right_click_executor),
            },
        };

        (context, components)
    }

    fn configure_click_executor(&self, button: MouseButton, executor: &Arc<ClickExecutor>) {
        let settings = Settings::load().unwrap_or_default();

        match button {
            MouseButton::Left => {
                executor.set_max_cps(settings.left_max_cps);
                let mode = match settings.left_game_mode.as_str() {
                    "Combo" => GameMode::Combo,
                    _ => GameMode::Default,
                };
                executor.set_game_mode(mode);
            }
            MouseButton::Right => {
                executor.set_max_cps(settings.right_max_cps);
                let mode = match settings.right_game_mode.as_str() {
                    "Combo" => GameMode::Combo,
                    _ => GameMode::Default,
                };
                executor.set_game_mode(mode);
            }
        }
    }

    fn is_mouse_button_pressed(&self, button: MouseButton) -> bool {
        unsafe {
            match button {
                MouseButton::Left => GetAsyncKeyState(0x01) < 0,
                MouseButton::Right => GetAsyncKeyState(0x02) < 0,
            }
        }
    }

    fn get_current_hwnd(&self) -> HWND {
        let hwnd_guard = self.hwnd.lock().unwrap();
        hwnd_guard.get()
    }

    fn get_next_delay(&self, delay_provider: &Arc<Mutex<DelayProvider>>) -> Duration {
        let mut provider = delay_provider.lock().unwrap();
        provider.get_next_delay()
    }

    fn spawn_click_thread(name: &str, service: Arc<ClickService>, button: MouseButton) {
        let context = format!("ClickService::{}", name);

        match thread::Builder::new()
            .name(name.to_string())
            .spawn(move || {
                service.click_loop(button);
            }) {
            Ok(_) => {
                log_info(&format!("{} spawned successfully", name), &context);
            }
            Err(e) => {
                log_error(&format!("Failed to spawn {}: {}", name, e), &context);
            }
        }
    }

    fn window_finder_loop(&self) {
        const CONTEXT: &str = "ClickService::window_finder_loop";

        log_info("Window finder thread started", CONTEXT);

        let running = Arc::new(AtomicBool::new(true));
        let running_clone = running.clone();

        let default_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |panic_info| {
            running_clone.store(false, Ordering::SeqCst);
            default_hook(panic_info);
        }));

        let mut consecutive_failures = 0;

        while running.load(Ordering::SeqCst) &&
            self.window_finder_running.load(Ordering::SeqCst) &&
            !thread::panicking() {
            let check_interval = self.get_window_check_interval();

            match std::panic::catch_unwind(AssertUnwindSafe(|| {
                self.window_finder.find_target_window(&self.hwnd)
            })) {
                Ok(_) => {
                    consecutive_failures = 0;
                    thread::sleep(check_interval);
                }
                Err(e) => {
                    consecutive_failures += 1;

                    let error_msg = if let Some(s) = e.downcast_ref::<String>() {
                        s.clone()
                    } else if let Some(s) = e.downcast_ref::<&str>() {
                        s.to_string()
                    } else {
                        "Unknown error".to_string()
                    };

                    log_error(&format!("Error finding window: {} (attempt {}/{})",
                                       error_msg, consecutive_failures, defaults::MAX_WINDOW_FIND_FAILURES), CONTEXT);

                    if consecutive_failures >= defaults::MAX_WINDOW_FIND_FAILURES {
                        log_error("Too many consecutive failures, backing off", CONTEXT);
                        thread::sleep(Duration::from_secs(10));
                        consecutive_failures = 0;
                    } else {
                        thread::sleep(Duration::from_millis(500));
                    }
                }
            }
        }

        log_info("Window finder thread terminated", CONTEXT);
    }

    fn get_window_check_interval(&self) -> Duration {
        if self.is_enabled() {
            self.config.window_check_active_interval
        } else {
            self.config.window_check_idle_interval
        }
    }

    fn settings_sync_loop(&self) {
        const CONTEXT: &str = "ClickService::settings_sync_loop";

        log_info("Settings synchronization thread started", CONTEXT);

        let running = Arc::new(AtomicBool::new(true));
        let running_clone = running.clone();

        let default_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |panic_info| {
            running_clone.store(false, Ordering::SeqCst);
            default_hook(panic_info);
        }));

        while running.load(Ordering::SeqCst) && !thread::panicking() {
            match std::panic::catch_unwind(AssertUnwindSafe(|| {
                self.check_and_update_settings();
            })) {
                Ok(_) => {
                    thread::sleep(Duration::from_secs(5));
                }
                Err(e) => {
                    let error_msg = if let Some(s) = e.downcast_ref::<String>() {
                        s.clone()
                    } else if let Some(s) = e.downcast_ref::<&str>() {
                        s.to_string()
                    } else {
                        "Unknown error".to_string()
                    };

                    log_error(&format!("Error during settings update: {}", error_msg), CONTEXT);

                    thread::sleep(Duration::from_secs(1));
                }
            }
        }

        log_error("Settings sync loop terminated", CONTEXT);
    }

    fn check_and_update_settings(&self) {
        let context = "ClickService::check_and_update_settings";

        match Settings::load() {
            Ok(new_settings) => {
                let current_settings = {
                    let settings = self.settings.lock().unwrap();
                    settings.clone()
                };

                let changes = SettingsChanges {
                    target_process: current_settings.target_process != new_settings.target_process,
                    adaptive_cpu_mode: current_settings.adaptive_cpu_mode != new_settings.adaptive_cpu_mode,
                };

                {
                    let mut settings = self.settings.lock().unwrap();
                    *settings = new_settings.clone();
                }

                if changes.target_process {
                    let _ = self.window_finder.update_target_process(&new_settings.target_process);
                }

                if changes.adaptive_cpu_mode {
                    self.left_thread_controller.set_adaptive_mode(new_settings.adaptive_cpu_mode);
                    self.right_thread_controller.set_adaptive_mode(new_settings.adaptive_cpu_mode);
                }
            }
            Err(e) => {
                log_error(&format!("Failed to reload settings: {}", e), context);
            }
        }
    }

    pub fn toggle(&self) -> bool {
        self.sync_controller.toggle()
    }

    pub fn is_enabled(&self) -> bool {
        self.sync_controller.is_enabled()
    }

    pub fn get_left_click_executor(&self) -> Arc<ClickExecutor> {
        Arc::clone(&self.left_click_executor)
    }

    pub fn get_right_click_executor(&self) -> Arc<ClickExecutor> {
        Arc::clone(&self.right_click_executor)
    }

    pub fn set_left_click_cps(&self, cps: u8) {
        self.left_click_executor.set_max_cps(cps);
    }

    pub fn set_right_click_cps(&self, cps: u8) {
        self.right_click_executor.set_max_cps(cps);
    }

    pub fn force_enable_clicking(&self) -> bool {
        if self.is_enabled() {
            return true;
        }

        self.sync_controller.force_enable()
    }

    pub fn force_disable_clicking(&self) -> bool {
        if !self.is_enabled() {
            return true;
        }

        if self.sync_controller.is_enabled() {
            self.sync_controller.toggle();
        }

        true
    }

    pub fn force_enable_left_clicking(&self) -> bool {
        if self.left_click_controller.is_enabled() {
            return true;
        }
        self.left_click_controller.force_enable()
    }

    pub fn force_enable_right_clicking(&self) -> bool {
        if self.right_click_controller.is_enabled() {
            return true;
        }
        self.right_click_controller.force_enable()
    }

    pub fn force_disable_left_clicking(&self) -> bool {
        if !self.left_click_controller.is_enabled() {
            return true;
        }
        self.left_click_controller.toggle()
    }

    pub fn force_disable_right_clicking(&self) -> bool {
        if !self.right_click_controller.is_enabled() {
            return true;
        }
        self.right_click_controller.toggle()
    }
}
