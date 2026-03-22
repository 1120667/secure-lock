// SecureLock — Lock Controller
// Manages the lock session lifecycle as a state machine.

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Instant;

/// Possible states of the lock session.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum LockPhase {
    Idle,
    Locking,
    Locked,
    Unlocking,
}

/// Information about the allowed application.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AllowedApp {
    pub hwnd: isize,
    pub title: String,
    pub process_name: String,
}

/// Internal session data.
#[derive(Debug, Clone)]
pub struct SessionData {
    pub allowed_app: AllowedApp,
    pub pin_hash: String,
    pub locked_at: Instant,
}

/// The full lock state — thread-safe behind a Mutex.
#[derive(Debug)]
pub struct LockState {
    pub status: LockPhase,
    pub allowed_apps: Vec<AllowedApp>,
    pub expected_pin_hash: Option<String>,
    pub session_start: Option<Instant>,
}

impl Default for LockState {
    fn default() -> Self {
        Self {
            status: LockPhase::Idle,
            allowed_apps: Vec::new(),
            expected_pin_hash: None,
            session_start: None,
        }
    }
}

/// Serializable lock status sent to the frontend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockStatus {
    pub status: LockPhase,
    pub allowed_apps: Vec<AllowedApp>,
    pub locked_duration_secs: Option<u64>,
}

/// Shared state handle used across Tauri commands.
pub type SharedLockState = Arc<Mutex<LockState>>;

/// Create a new shared lock state.
pub fn new_shared_state() -> SharedLockState {
    Arc::new(Mutex::new(LockState::default()))
}

/// Attempt to transition from Idle → Locking → Locked.
pub fn start_lock(
    state: &SharedLockState,
    apps: Vec<AllowedApp>,
    pin_hash: String,
) -> Result<(), String> {
    let mut st = state.lock();

    if st.status != LockPhase::Idle {
        return Err(format!(
            "Cannot start lock: current phase is {:?}",
            st.status
        ));
    }

    st.status = LockPhase::Locked;
    st.allowed_apps = apps;
    st.expected_pin_hash = Some(pin_hash);
    st.session_start = Some(Instant::now());

    log::info!("Lock session started");
    Ok(())
}

/// Attempt to transition from Locked → Unlocking → Idle.
pub fn end_lock(state: &SharedLockState) -> Result<(), String> {
    let mut st = state.lock();

    if st.status != LockPhase::Locked {
        return Err(format!(
            "Cannot unlock: current phase is {:?}",
            st.status
        ));
    }

    st.status = LockPhase::Idle;
    st.allowed_apps.clear();
    st.expected_pin_hash = None;
    st.session_start = None;

    log::info!("Lock session ended");
    Ok(())
}

/// Get the current lock status for the frontend.
pub fn get_status(state: &SharedLockState) -> LockStatus {
    let st = state.lock();
    LockStatus {
        status: st.status.clone(),
        allowed_apps: st.allowed_apps.clone(),
        locked_duration_secs: st.session_start.map(|start_time| {
            Instant::now()
                .duration_since(start_time)
                .as_secs()
        }),
    }
}

/// Get the stored PIN hash (for verification).
pub fn get_pin_hash(state: &SharedLockState) -> Option<String> {
    let st = state.lock();
    st.expected_pin_hash.clone()
}

/// Get the allowed app HWND.
// Remove get_allowed_hwnd entirely since window manager now needs the full array
// pub fn get_allowed_hwnd(state: &SharedLockState) -> Option<isize> {
//     let lock = state.lock();
//     lock.session.as_ref().map(|s| s.allowed_app.hwnd)
// }

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_app() -> AllowedApp {
        AllowedApp {
            hwnd: 12345,
            title: "Test App".to_string(),
            process_name: "test.exe".to_string(),
        }
    }

    #[test]
    fn test_initial_state() {
        let state = new_shared_state();
        let status = get_status(&state);
        assert_eq!(status.phase, LockPhase::Idle);
        assert!(status.allowed_app_title.is_none());
    }

    #[test]
    fn test_lock_unlock_cycle() {
        let state = new_shared_state();
        start_lock(&state, make_test_app(), "hash123".to_string()).unwrap();

        let status = get_status(&state);
        assert_eq!(status.phase, LockPhase::Locked);
        assert_eq!(status.allowed_app_title, Some("Test App".to_string()));

        end_lock(&state).unwrap();
        let status = get_status(&state);
        assert_eq!(status.phase, LockPhase::Idle);
    }

    #[test]
    fn test_double_lock_fails() {
        let state = new_shared_state();
        start_lock(&state, make_test_app(), "hash123".to_string()).unwrap();
        let result = start_lock(&state, make_test_app(), "hash456".to_string());
        assert!(result.is_err());
    }

    #[test]
    fn test_unlock_when_idle_fails() {
        let state = new_shared_state();
        let result = end_lock(&state);
        assert!(result.is_err());
    }
}
