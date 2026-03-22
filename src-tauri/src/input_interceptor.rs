// SecureLock — Input Interceptor
// Installs a low-level keyboard hook to block system shortcuts while locked.
// Windows-only implementation using `windows` crate v0.52.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

#[cfg(target_os = "windows")]
use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
#[cfg(target_os = "windows")]
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, DispatchMessageW, GetMessageW, SetWindowsHookExW,
    UnhookWindowsHookEx, HHOOK, KBDLLHOOKSTRUCT, MSG, WH_KEYBOARD_LL,
    WM_KEYDOWN, WM_SYSKEYDOWN, LLKHF_ALTDOWN,
};
#[cfg(target_os = "windows")]
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, VK_CONTROL, VK_ESCAPE, VK_F4, VK_LWIN,
    VK_RWIN, VK_SHIFT, VK_TAB,
};
#[cfg(target_os = "windows")]
use std::ptr::addr_of_mut;

#[cfg(target_os = "windows")]
static mut HOOK_HANDLE: Option<HHOOK> = None;
#[cfg(target_os = "windows")]
static mut HOOK_ACTIVE: bool = false;

/// Install the low-level keyboard hook on a dedicated thread.
/// Returns the thread handle.
#[cfg(target_os = "windows")]
pub fn install_hook(running: Arc<AtomicBool>) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        unsafe {
            let hook = SetWindowsHookExW(
                WH_KEYBOARD_LL,
                Some(keyboard_hook_proc),
                None,
                0,
            );

            match hook {
                Ok(h) => {
                    HOOK_HANDLE = Some(h);
                    HOOK_ACTIVE = true;
                    log::info!("Keyboard hook installed");

                    // Message loop — required for the hook to work
                    let mut msg = MSG::default();
                    while running.load(Ordering::Relaxed) {
                        let result = GetMessageW(&mut msg, None, 0, 0);
                        if !result.as_bool() {
                            break;
                        }
                        let _ = DispatchMessageW(&msg);
                    }

                    // Safety: single-threaded hook thread, no other references
                    let hook_ptr = addr_of_mut!(HOOK_HANDLE);
                    if let Some(h) = (*hook_ptr).take() {
                        let _ = UnhookWindowsHookEx(h);
                    }
                    let active_ptr = addr_of_mut!(HOOK_ACTIVE);
                    *active_ptr = false;
                    log::info!("Keyboard hook removed");
                }
                Err(e) => {
                    log::error!("Failed to install keyboard hook: {:?}", e);
                }
            }
        }
    })
}

/// Uninstall the keyboard hook.
#[cfg(target_os = "windows")]
pub fn uninstall_hook() {
    unsafe {
        let hook_ptr = addr_of_mut!(HOOK_HANDLE);
        if let Some(h) = (*hook_ptr).take() {
            let _ = UnhookWindowsHookEx(h);
            let active_ptr = addr_of_mut!(HOOK_ACTIVE);
            *active_ptr = false;
            log::info!("Keyboard hook uninstalled");
        }
    }
}

/// Check if Alt is pressed based on hook flags.
#[cfg(target_os = "windows")]
fn is_alt_down(flags: u32) -> bool {
    (flags & LLKHF_ALTDOWN.0) != 0
}

/// Check if Ctrl key is currently pressed.
#[cfg(target_os = "windows")]
fn is_ctrl_pressed() -> bool {
    unsafe { (GetAsyncKeyState(VK_CONTROL.0 as i32) & 0x8000u16 as i16) != 0 }
}

/// Check if Shift key is currently pressed.
#[cfg(target_os = "windows")]
fn is_shift_pressed() -> bool {
    unsafe { (GetAsyncKeyState(VK_SHIFT.0 as i32) & 0x8000u16 as i16) != 0 }
}

/// Low-level keyboard hook callback.
/// Blocks dangerous system shortcuts.
#[cfg(target_os = "windows")]
unsafe extern "system" fn keyboard_hook_proc(
    code: i32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if code >= 0 && HOOK_ACTIVE {
        let kb = *(lparam.0 as *const KBDLLHOOKSTRUCT);
        let vk = kb.vkCode;
        let flags = kb.flags.0;
        let is_keydown =
            wparam.0 == WM_KEYDOWN as usize || wparam.0 == WM_SYSKEYDOWN as usize;

        if is_keydown {
            // Block Win keys (left and right)
            if vk == VK_LWIN.0 as u32 || vk == VK_RWIN.0 as u32 {
                log::debug!("Blocked Win key");
                return LRESULT(1);
            }

            // Block Alt+Tab
            if vk == VK_TAB.0 as u32 && is_alt_down(flags) {
                log::debug!("Blocked Alt+Tab");
                return LRESULT(1);
            }

            // Block Alt+F4
            if vk == VK_F4.0 as u32 && is_alt_down(flags) {
                log::debug!("Blocked Alt+F4");
                return LRESULT(1);
            }

            // Block Alt+Esc
            if vk == VK_ESCAPE.0 as u32 && is_alt_down(flags) {
                log::debug!("Blocked Alt+Esc");
                return LRESULT(1);
            }

            // Block Ctrl+Esc (Start Menu)
            if vk == VK_ESCAPE.0 as u32 && is_ctrl_pressed() {
                log::debug!("Blocked Ctrl+Esc");
                return LRESULT(1);
            }

            // Block Ctrl+Shift+Esc (Task Manager)
            if vk == VK_ESCAPE.0 as u32 && is_ctrl_pressed() && is_shift_pressed() {
                log::debug!("Blocked Ctrl+Shift+Esc");
                return LRESULT(1);
            }
        }
    }

    CallNextHookEx(None, code, wparam, lparam)
}

// Non-windows stubs
#[cfg(not(target_os = "windows"))]
pub fn install_hook(_running: Arc<AtomicBool>) -> std::thread::JoinHandle<()> {
    log::warn!("Keyboard hook not supported on this platform");
    std::thread::spawn(|| {})
}

#[cfg(not(target_os = "windows"))]
pub fn uninstall_hook() {
    log::warn!("Keyboard hook not supported on this platform");
}
