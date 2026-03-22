// SecureLock — Window Morpher (formerly Overlay Guard)
// Morphs the main setup window into a floating, draggable lock panel.
// This prevents Webview2 creation bugs (white screens) by reusing the existing window.

use tauri::{AppHandle, Manager, LogicalSize, Size, LogicalPosition, Position};

/// Shrink the main window, remove decorations, and switch to the locked view.
pub fn morph_to_overlay(app: &AppHandle) -> Result<(), String> {
    if let Some(win) = app.get_webview_window("main") {
        // Strip window decorations
        win.set_decorations(false).map_err(|e| e.to_string())?;
        win.set_always_on_top(true).map_err(|e| e.to_string())?;
        win.set_skip_taskbar(true).map_err(|e| e.to_string())?;
        win.set_resizable(false).map_err(|e| e.to_string())?;
        
        // Resize to tiny floating widget size
        let _ = win.set_size(Size::Logical(LogicalSize { width: 280.0, height: 72.0 }));
        
        // Position at Top Right
        if let Ok(Some(monitor)) = win.current_monitor() {
            let scale_factor = monitor.scale_factor();
            let monitor_size = monitor.size();
            let logical_width = monitor_size.width as f64 / scale_factor;
            
            let x = logical_width - 280.0 - 24.0; // 24px padding from right
            let y = 24.0; // 24px padding from top
            let _ = win.set_position(Position::Logical(LogicalPosition { x, y }));
        } else {
            win.center().unwrap_or(());
        }
    }
    
    log::info!("Morphed main window to overlay panel");
    Ok(())
}

/// Restore the main window to its setup state.
pub fn restore_main_window(app: &AppHandle) -> Result<(), String> {
    if let Some(win) = app.get_webview_window("main") {
        win.set_decorations(true).map_err(|e| e.to_string())?;
        win.set_always_on_top(false).map_err(|e| e.to_string())?;
        win.set_skip_taskbar(false).map_err(|e| e.to_string())?;
        win.set_resizable(false).map_err(|e| e.to_string())?;
        
        // Restore setup size
        let _ = win.set_size(Size::Logical(LogicalSize { width: 520.0, height: 680.0 }));
        win.center().map_err(|e| e.to_string())?;
        
        // Ensure it has focus
        win.show().unwrap_or(());
        win.set_focus().unwrap_or(());
    }
    
    log::info!("Restored main window");
    Ok(())
}

/// Periodically called to ensure the lock panel stays on top
pub fn enforce_overlay_top(app: &AppHandle) {
    if let Some(win) = app.get_webview_window("main") {
        // Force unminimize if the user triggered "Show Desktop" (Win+D)
        if let Ok(minimized) = win.is_minimized() {
            if minimized {
                let _ = win.unminimize();
            }
        }
        let _ = win.show();
        let _ = win.set_always_on_top(true);
        
        // Tauri's always_on_top caches state and can be overpowered by SetForegroundWindow.
        // We must aggressively re-assert HWND_TOPMOST via raw Win32 APIs every tick.
        #[cfg(target_os = "windows")]
        unsafe {
            use windows::Win32::UI::WindowsAndMessaging::{FindWindowW, SetWindowPos, HWND_TOPMOST, SWP_NOMOVE, SWP_NOSIZE};
            use windows::core::PCWSTR;
            let mut title_buf: Vec<u16> = "SecureLock".encode_utf16().chain(std::iter::once(0)).collect();
            let overlay_hwnd = FindWindowW(PCWSTR::null(), PCWSTR(title_buf.as_ptr()));
            if overlay_hwnd.0 != 0 {
                let _ = SetWindowPos(overlay_hwnd, HWND_TOPMOST, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE);
            }
        }
    }
}
