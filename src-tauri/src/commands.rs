// SecureLock — Tauri IPC Commands
// These are the functions exposed to the frontend via `invoke()`.

use crate::lock_controller::{self, AllowedApp, LockStatus, SharedLockState};
use crate::security_module::{self, SharedRateLimiter};
use crate::window_manager::{self, SavedStates, WindowInfo};
use crate::overlay_guard;
use crate::input_interceptor;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use parking_lot::Mutex;
use tauri::{AppHandle, Manager, State};

/// Managed state for all subsystems.
pub struct AppState {
    pub lock_state: SharedLockState,
    pub rate_limiter: SharedRateLimiter,
    pub saved_window_states: SavedStates,
    pub enforcement_running: Arc<AtomicBool>,
    pub hook_running: Arc<AtomicBool>,
    pub enforcement_thread: Mutex<Option<std::thread::JoinHandle<()>>>,
    pub hook_thread: Mutex<Option<std::thread::JoinHandle<()>>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            lock_state: lock_controller::new_shared_state(),
            rate_limiter: security_module::new_rate_limiter(),
            saved_window_states: window_manager::new_saved_states(),
            enforcement_running: Arc::new(AtomicBool::new(false)),
            hook_running: Arc::new(AtomicBool::new(false)),
            enforcement_thread: Mutex::new(None),
            hook_thread: Mutex::new(None),
        }
    }
}

/// Get list of running applications with visible windows.
#[tauri::command]
pub fn get_running_apps() -> Vec<WindowInfo> {
    window_manager::enumerate_windows()
}

/// Start a lock session.
#[tauri::command]
pub fn start_lock(
    app: AppHandle,
    state: State<'_, AppState>,
    apps: Vec<AllowedApp>,
    pin: String,
) -> Result<String, String> {
    // Validate PIN
    if pin.len() < 4 || pin.len() > 8 {
        return Err("PIN must be between 4 and 8 characters".to_string());
    }
    
    // Validate Apps
    if apps.is_empty() {
        return Err("Must select at least one application".to_string());
    }

    // Hash the PIN
    let pin_hash = security_module::hash_pin(&pin)?;

    // Extract hwnds for window manager
    let hwnds: Vec<isize> = apps.iter().map(|a| a.hwnd).collect();
    let title = if apps.len() == 1 { apps[0].title.clone() } else { format!("{} Applications", apps.len()) };

    // Transition lock state
    lock_controller::start_lock(&state.lock_state, apps, pin_hash)?;

    // Hide other windows
    window_manager::hide_other_windows(&hwnds, &state.saved_window_states);

    // Install keyboard hook
    state.hook_running.store(true, Ordering::Relaxed);
    let hook_running = state.hook_running.clone();
    let handle = input_interceptor::install_hook(hook_running);
    *state.hook_thread.lock() = Some(handle);

    // Start window enforcement thread
    state.enforcement_running.store(true, Ordering::Relaxed);
    let enforcement_running = state.enforcement_running.clone();
    let handle = window_manager::start_enforcement_thread(hwnds, enforcement_running, app.clone());
    *state.enforcement_thread.lock() = Some(handle);

    // Morph main window into the overlay widget
    overlay_guard::morph_to_overlay(&app)?;

    log::info!("Lock session started for: {}", title);
    Ok(format!("Locked to: {}", title))
}

/// Attempt to unlock with a PIN.
#[tauri::command]
pub fn attempt_unlock(
    app: AppHandle,
    state: State<'_, AppState>,
    pin: String,
) -> Result<bool, String> {
    // Get stored hash
    let stored_hash = lock_controller::get_pin_hash(&state.lock_state)
        .ok_or_else(|| "No active lock session".to_string())?;

    // Attempt verification with rate limiting
    let is_valid = security_module::attempt_unlock(&pin, &stored_hash, &state.rate_limiter)?;

    if is_valid {
        // Stop enforcement threads
        state.enforcement_running.store(false, Ordering::Relaxed);
        state.hook_running.store(false, Ordering::Relaxed);

        // Uninstall keyboard hook
        input_interceptor::uninstall_hook();

        // End lock session before morphing back to avoid race conditions
        lock_controller::end_lock(&state.lock_state)?;

        // Restore windows
        window_manager::restore_windows(&state.saved_window_states);

        // Morph overlay widget back into main window
        overlay_guard::restore_main_window(&app)?;

        log::info!("Unlock successful");
        Ok(true)
    } else {
        let remaining = security_module::get_failed_attempts(&state.rate_limiter);
        log::warn!("Unlock failed. Attempts: {}", remaining);
        Ok(false)
    }
}

/// Get current lock status.
#[tauri::command]
pub fn get_lock_status(state: State<'_, AppState>) -> LockStatus {
    lock_controller::get_status(&state.lock_state)
}

/// Get rate limiter info (failed attempts and cooldown).
#[tauri::command]
pub fn get_rate_limit_info(state: State<'_, AppState>) -> (u32, u64) {
    let attempts = security_module::get_failed_attempts(&state.rate_limiter);
    let cooldown = security_module::get_cooldown_remaining(&state.rate_limiter);
    (attempts, cooldown)
}

/// Dynamically resize the window.
#[tauri::command]
pub fn resize_overlay_widget(app: AppHandle, width: f64, height: f64) -> Result<(), String> {
    if let Some(win) = app.get_webview_window("main") {
        use tauri::{Size, LogicalSize};
        let _ = win.set_size(Size::Logical(LogicalSize { width, height }));
    }
    Ok(())
}
