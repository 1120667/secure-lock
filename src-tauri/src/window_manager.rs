// SecureLock — Window Manager
// Enumerates visible windows, pins the allowed app, hides others, restores on unlock.
// Windows-only implementation using the `windows` crate v0.52.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use parking_lot::Mutex;

#[cfg(target_os = "windows")]
use windows::Win32::Foundation::{BOOL, HWND, LPARAM};
#[cfg(target_os = "windows")]
use windows::core::PCWSTR;
#[cfg(target_os = "windows")]
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetForegroundWindow, GetWindowTextLengthW, GetWindowTextW,
    GetWindowThreadProcessId, IsWindowVisible, SetForegroundWindow, ShowWindow,
    SW_MINIMIZE, SW_RESTORE, SW_MAXIMIZE, SW_HIDE, SW_SHOW, GetWindowLongW, GWL_STYLE,
    WS_VISIBLE, WS_CAPTION, GetClassNameW, FindWindowW, FindWindowExW,
};
#[cfg(target_os = "windows")]
use windows::Win32::UI::Input::KeyboardAndMouse::EnableWindow;
#[cfg(target_os = "windows")]
use windows::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_FORMAT, PROCESS_QUERY_LIMITED_INFORMATION,
};

/// Represents a visible window on the system.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowInfo {
    pub hwnd: isize,
    pub title: String,
    pub process_name: String,
    pub class_name: String,
}

/// Saved window state for restoration.
#[derive(Debug, Clone)]
pub struct SavedWindowState {
    pub hwnd: isize,
}

/// Shared storage for saved window states.
pub type SavedStates = Arc<Mutex<HashMap<isize, SavedWindowState>>>;

/// Create a new saved states store.
pub fn new_saved_states() -> SavedStates {
    Arc::new(Mutex::new(HashMap::new()))
}

/// Helper to create HWND from isize.
#[cfg(target_os = "windows")]
fn make_hwnd(val: isize) -> HWND {
    HWND(val)
}

#[cfg(target_os = "windows")]
fn encode_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Toggles visibility of desktop icons and disables right-clicks on the desktop
#[cfg(target_os = "windows")]
pub fn toggle_desktop(show: bool) {
    unsafe {
        let progman_class = encode_wide("Progman");
        let workerw_class = encode_wide("WorkerW");
        let shelldll_class = encode_wide("SHELLDLL_DefView");
        let syslist_class = encode_wide("SysListView32");
        let folderview_title = encode_wide("FolderView");

        let mut def_view = HWND(0);
        
        let progman = FindWindowW(PCWSTR(progman_class.as_ptr()), PCWSTR::null());
        if progman.0 != 0 {
            def_view = FindWindowExW(progman, HWND(0), PCWSTR(shelldll_class.as_ptr()), PCWSTR::null());
        }
        
        if def_view.0 == 0 {
            let mut worker = FindWindowW(PCWSTR(workerw_class.as_ptr()), PCWSTR::null());
            while worker.0 != 0 && def_view.0 == 0 {
                def_view = FindWindowExW(worker, HWND(0), PCWSTR(shelldll_class.as_ptr()), PCWSTR::null());
                worker = FindWindowExW(HWND(0), worker, PCWSTR(workerw_class.as_ptr()), PCWSTR::null());
            }
        }

        if def_view.0 != 0 {
            let list_view = FindWindowExW(def_view, HWND(0), PCWSTR(syslist_class.as_ptr()), PCWSTR(folderview_title.as_ptr()));
            let show_cmd = if show { SW_SHOW } else { SW_HIDE };
            
            if list_view.0 != 0 {
                let _ = ShowWindow(list_view, show_cmd);
            }
            
            // Enable or format the Desktop to block input (right click)
            let _ = EnableWindow(def_view, show);
        }
        
        // Also disable the Taskbar entirely to prevent Start Menu or System Tray interactions
        let taskbar_class = encode_wide("Shell_TrayWnd");
        let taskbar = FindWindowW(PCWSTR(taskbar_class.as_ptr()), PCWSTR::null());
        if taskbar.0 != 0 {
            let _ = EnableWindow(taskbar, show);
        }
    }
}

/// Enumerate all visible, titled windows on the system.
#[cfg(target_os = "windows")]
pub fn enumerate_windows() -> Vec<WindowInfo> {
    let windows: Arc<Mutex<Vec<WindowInfo>>> = Arc::new(Mutex::new(Vec::new()));
    let windows_clone = windows.clone();

    unsafe {
        let _ = EnumWindows(
            Some(enum_window_proc),
            LPARAM(&*windows_clone as *const Mutex<Vec<WindowInfo>> as isize),
        );
    }

    let result = windows.lock().clone();
    result
}

#[cfg(target_os = "windows")]
unsafe extern "system" fn enum_window_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
    // Skip invisible windows
    if !IsWindowVisible(hwnd).as_bool() {
        return BOOL(1);
    }

    // Skip windows without a title
    let title_len = GetWindowTextLengthW(hwnd);
    if title_len == 0 {
        return BOOL(1);
    }

    // Skip windows without proper style (no caption = internal windows)
    let style_val = GetWindowLongW(hwnd, GWL_STYLE) as u32;
    let has_visible = (style_val & WS_VISIBLE.0) != 0;
    let has_caption = (style_val & WS_CAPTION.0) != 0;
    if !has_visible || !has_caption {
        return BOOL(1);
    }

    // Get window title
    let mut title_buf = vec![0u16; (title_len + 1) as usize];
    let len = GetWindowTextW(hwnd, &mut title_buf);
    let title = String::from_utf16_lossy(&title_buf[..len as usize]);

    // Skip empty titles and our own window
    if title.is_empty() || title == "SecureLock" {
        return BOOL(1);
    }

    // Get process name
    let mut process_id: u32 = 0;
    GetWindowThreadProcessId(hwnd, Some(&mut process_id));
    let process_name = get_process_name(process_id);

    // Get class name
    let mut class_buf = vec![0u16; 256];
    let class_len = GetClassNameW(hwnd, &mut class_buf);
    let class_name = String::from_utf16_lossy(&class_buf[..class_len as usize]);

    let windows_ptr = lparam.0 as *const Mutex<Vec<WindowInfo>>;
    if let Some(windows) = windows_ptr.as_ref() {
        let mut list = windows.lock();
        list.push(WindowInfo {
            hwnd: hwnd.0 as isize,
            title,
            process_name,
            class_name,
        });
    }

    BOOL(1)
}

#[cfg(target_os = "windows")]
fn get_process_name(process_id: u32) -> String {
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, process_id);
        match handle {
            Ok(h) => {
                let mut buf = vec![0u16; 260];
                let mut size = buf.len() as u32;
                let result = QueryFullProcessImageNameW(
                    h,
                    PROCESS_NAME_FORMAT(0),
                    windows::core::PWSTR(buf.as_mut_ptr()),
                    &mut size,
                );
                if result.is_ok() {
                    let full_path = String::from_utf16_lossy(&buf[..size as usize]);
                    full_path
                        .rsplit('\\')
                        .next()
                        .unwrap_or("unknown")
                        .to_string()
                } else {
                    "unknown".to_string()
                }
            }
            Err(_) => "unknown".to_string(),
        }
    }
}

#[cfg(target_os = "windows")]
fn get_window_title(hwnd: HWND) -> String {
    unsafe {
        let title_len = GetWindowTextLengthW(hwnd);
        if title_len == 0 {
            return String::new();
        }
        let mut title_buf = vec![0u16; (title_len + 1) as usize];
        let len = GetWindowTextW(hwnd, &mut title_buf);
        String::from_utf16_lossy(&title_buf[..len as usize])
    }
}

#[cfg(target_os = "windows")]
fn get_window_class_name(hwnd: HWND) -> String {
    unsafe {
        let mut class_buf = vec![0u16; 256];
        let len = GetClassNameW(hwnd, &mut class_buf);
        String::from_utf16_lossy(&class_buf[..len as usize])
    }
}

#[cfg(target_os = "windows")]
fn enumerate_window_handles() -> Vec<isize> {
    enumerate_windows().into_iter().map(|w| w.hwnd).collect()
}

/// Minimize all windows except the allowed one. Save their state for later restoration.
#[cfg(target_os = "windows")]
pub fn hide_other_windows(allowed_hwnds: &[isize], saved_states: &SavedStates) {
    let mut states = saved_states.lock();
    states.clear();

    let all_windows = enumerate_windows();
    
    // We only need hwnds of all_windows, but we will iterate over full WindowInfo to filter
    // Wait, let's just use all_windows directly.
    
    for win in all_windows {
        let hwnd = win.hwnd;

        // Skip allowed apps
        if allowed_hwnds.contains(&hwnd) {
            continue;
        }
        // Skip our own window
        if win.title.contains("SecureLock") {
            continue;
        }
        // Safety: We must not hide OS Shell windows (Taskbar, Desktop, Start Menu, System Tray Popups)
        let proc = win.process_name.to_lowercase();
        let cls = win.class_name.to_lowercase();
        
        if proc == "explorer.exe" {
            // "CabinetWClass" and "ExploreWClass" are actual File Explorer folder windows.
            // Everything else in explorer.exe is a system UI component.
            if cls != "cabinetwclass" && cls != "explorewclass" {
                continue;
            }
        } else if proc == "searchapp.exe"
            || proc == "searchhost.exe"
            || proc == "shellexperiencehost.exe"
            || proc == "textinputhost.exe" {
            continue;
        }

        states.insert(
            hwnd,
            SavedWindowState {
                hwnd,
            },
        );
        unsafe {
            let _ = ShowWindow(make_hwnd(hwnd), SW_MINIMIZE);
        }
    }

    // Hide Desktop icons and disable right-click interactions
    toggle_desktop(false);
    
    // Bring all allowed apps to foreground and maximize them
    for &hwnd in allowed_hwnds {
        unsafe {
            let _ = ShowWindow(make_hwnd(hwnd), SW_MAXIMIZE);
            let _ = SetForegroundWindow(make_hwnd(hwnd));
        }
    }

    log::info!(
        "Hidden {} windows, pinned hwnds={:?}",
        states.len(),
        allowed_hwnds
    );
}

/// Restore all previously hidden windows.
#[cfg(target_os = "windows")]
pub fn restore_windows(saved_states: &SavedStates) {
    let mut states = saved_states.lock();

    for (_, saved) in states.iter() {
        unsafe {
            let _ = ShowWindow(make_hwnd(saved.hwnd), SW_RESTORE);
        }
    }

    // Restore desktop icons and functionality
    toggle_desktop(true);

    let count = states.len();
    states.clear();
    log::info!("Restored {} windows", count);
}

/// Re-focus the allowed window (called periodically to enforce pinning).
#[cfg(target_os = "windows")]
pub fn enforce_focus(allowed_hwnds: &[isize]) {
    unsafe {
        let fg = GetForegroundWindow();
        if fg.0 == 0 {
            return;
        }
        let fg_isize = fg.0 as isize;

        // If the foreground window is not in our allowed list, we might need to intervene.
        if !allowed_hwnds.contains(&fg_isize) {
            let title = get_window_title(fg);
            // Allow SecureLock Overlay to have focus
            if title.contains("SecureLock") {
                return;
            }
            
            // Allow critical OS popups (Start Menu/Network/etc) to momentarily steal focus
            // BUT do not allow Desktop ("Progman" / "WorkerW") or File Explorer ("CabinetWClass")
            let cls = get_window_class_name(fg).to_lowercase();
            let mut process_id: u32 = 0;
            GetWindowThreadProcessId(fg, Some(&mut process_id));
            let proc = get_process_name(process_id).to_lowercase();
            
            if proc == "explorer.exe" {
                if cls != "cabinetwclass" && cls != "explorewclass" && cls != "progman" && cls != "workerw" {
                    return; // Let Taskbar/Start Menu steal focus
                }
            } else if proc == "textinputhost.exe" 
                || proc == "searchapp.exe" 
                || proc == "searchhost.exe" 
                || proc == "shellexperiencehost.exe" {
                return;
            }

            // Some other app got focus — re-minimize and refocus allowed apps
            let _ = ShowWindow(fg, SW_MINIMIZE);
            // We just bring the first allowed app to the absolute forefront to steal focus safely
            if let Some(&first_hwnd) = allowed_hwnds.first() {
                let _ = ShowWindow(make_hwnd(first_hwnd), SW_MAXIMIZE);
                let _ = SetForegroundWindow(make_hwnd(first_hwnd));
            }
        }
    }
}

/// Start a background thread that periodically enforces window focus.
#[cfg(target_os = "windows")]
pub fn start_enforcement_thread(
    allowed_hwnds: Vec<isize>,
    running: Arc<AtomicBool>,
    app: tauri::AppHandle,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        while running.load(Ordering::Relaxed) {
            crate::overlay_guard::enforce_overlay_top(&app);
            enforce_focus(&allowed_hwnds);
            
            // Also minimize any new windows that appeared
            let windows = enumerate_windows();
            for win in windows {
                let hwnd = win.hwnd;
                if !allowed_hwnds.contains(&hwnd) {
                    if !win.title.contains("SecureLock") {
                        let proc = win.process_name.to_lowercase();
                        let cls = win.class_name.to_lowercase();
                        
                        if proc == "explorer.exe" {
                            if cls != "cabinetwclass" && cls != "explorewclass" {
                                continue;
                            }
                        } else if proc == "searchapp.exe" 
                            || proc == "searchhost.exe" 
                            || proc == "shellexperiencehost.exe" 
                            || proc == "textinputhost.exe" {
                            continue;
                        }

                        unsafe {
                            let _ = ShowWindow(make_hwnd(hwnd), SW_MINIMIZE);
                        }
                    }
                }
            }
            
            std::thread::sleep(std::time::Duration::from_millis(500));
        }
    })
}

// Non-windows stubs for compilation on other platforms
#[cfg(not(target_os = "windows"))]
pub fn enumerate_windows() -> Vec<WindowInfo> {
    log::warn!("Window enumeration not supported on this platform");
    Vec::new()
}

#[cfg(not(target_os = "windows"))]
pub fn hide_other_windows(_allowed_hwnds: &[isize], _saved_states: &SavedStates) {
    log::warn!("Window hiding not supported on this platform");
}

#[cfg(not(target_os = "windows"))]
pub fn restore_windows(_saved_states: &SavedStates) {
    log::warn!("Window restore not supported on this platform");
}

#[cfg(not(target_os = "windows"))]
pub fn toggle_desktop(_show: bool) {
    log::warn!("Desktop toggling not supported on this platform");
}

#[cfg(not(target_os = "windows"))]
pub fn enforce_focus(_allowed_hwnds: &[isize]) {}

#[cfg(not(target_os = "windows"))]
pub fn start_enforcement_thread(
    _allowed_hwnds: Vec<isize>,
    _running: Arc<AtomicBool>,
    _app: tauri::AppHandle,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(|| {})
}
