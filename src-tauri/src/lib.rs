use tauri::Manager;

mod autostart;
mod browsers;
mod checks;
mod commands;
mod config;
mod diag;
mod geo;
mod engines;
mod i18n;
mod log;
mod privileged;
mod recording;
mod snapshot;
mod tray;
mod uninstall;

/// EN: Handles a launch with `--privileged` (see privileged.rs) or `--uninstall` (see uninstall.rs).
///     None — a normal launch with a window.
/// RU: Обработка запуска с `--privileged` (см. privileged.rs) или `--uninstall` (см. uninstall.rs).
///     None — обычный запуск с окном.
pub fn handle_privileged_args() -> Option<i32> {
    privileged::handle_args().or_else(uninstall::handle_args)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // EN: A second launch (shortcut while the app sits in the tray) only shows the window of the first one.
        // RU: Второй запуск (ярлык, пока приложение в трее) только показывает окно первого.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| tray::show_window(app)))
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(commands::PickState::default())
        .manage(recording::RecordingState::default())
        .manage(diag::DiagState::default())
        .manage(geo::CountryState::default())
        .setup(|app| {
            recording::clean_up_on_start();
            tray::create(app.handle())?;
            geo::spawn_watcher(app.handle().clone());
            // EN: The window is hidden in the config: started by the autostart task the app stays in the tray.
            // RU: Окно скрыто в настройках: запущенное задачей автозапуска приложение остаётся в трее.
            let is_tray_start = std::env::args().any(|arg| arg == autostart::TRAY_FLAG);
            if !is_tray_start {
                tray::show_window(app.handle());
            }
            diag::clean_up_on_start(app.handle());
            let config = config::load(app.handle());
            let language = config.language.unwrap_or_else(i18n::system_lang);
            i18n::set_lang(language);
            crate::info!(
                "app start: lang={language:?} zapret_dir={:?} rollback_dir={:?} | {}",
                config.zapret_dir,
                config.rollback_dir,
                engines::zapret::service::describe_for_log()
            );
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event
                && window.label() == "main"
                && config::load(window.app_handle()).close_to_tray
            {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_language,
            commands::set_language,
            commands::get_zapret_overview,
            commands::get_latest_release,
            commands::download_zapret,
            commands::set_zapret_dir,
            commands::forget_zapret_dir,
            commands::check_zapret_update,
            commands::zapret_update,
            commands::zapret_rollback,
            commands::zapret_install,
            commands::zapret_start,
            commands::zapret_stop,
            commands::zapret_remove,
            commands::get_autostart,
            commands::set_autostart_mode,
            commands::get_app_options,
            commands::set_app_options,
            commands::get_country,
            commands::check_services,
            commands::get_last_pick,
            commands::zapret_quick_pick,
            commands::zapret_cancel_pick,
            commands::log_recording_status,
            commands::log_recording_start,
            commands::log_recording_stop,
            commands::log_ui,
            commands::diag_video_check,
            commands::diag_video_cancel,
            commands::diag_keep_watching,
            commands::get_last_deep_pick,
            commands::diag_deep_pick,
            commands::get_browsers_quic,
            commands::set_browsers_quic,
            commands::get_zapret_services,
            commands::set_zapret_service,
            commands::add_custom_domain,
            commands::remove_custom_domain,
            commands::add_user_exclusion,
            commands::remove_user_exclusion,
            commands::dismiss_fresh_domains,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
