// SecureLock — Application Entry Point
// Initializes the Tauri app with all managed state and commands.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod input_interceptor;
mod lock_controller;
mod overlay_guard;
mod security_module;
mod window_manager;

use commands::AppState;

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .init();

    log::info!("SecureLock starting...");

    tauri::Builder::default()
        .manage(AppState::new())
        .invoke_handler(tauri::generate_handler![
            commands::get_running_apps,
            commands::start_lock,
            commands::attempt_unlock,
            commands::get_lock_status,
            commands::get_rate_limit_info,
            commands::resize_overlay_widget
        ])
        .run(tauri::generate_context!())
        .expect("Error running SecureLock");
}
