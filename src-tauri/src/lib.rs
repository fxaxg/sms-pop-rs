//! SmsPop 主程序。
//!
//! 组装：插件 → 全局状态 → 托盘 → 蓝牙链路线程 → 命令。

mod app_launcher;
#[cfg(windows)]
mod caret;
mod commands;
mod devices;
mod http_ingress;
#[cfg(windows)]
mod link_worker;
#[cfg(target_os = "macos")]
mod macos_input;
mod notification_dispatch;
mod paths;
mod platform;
mod popups;
#[cfg(target_os = "macos")]
mod shortcut;
mod state;
mod token_store;
mod tray;
mod types;
mod updater;

use log::info;
use smspop_core::config::Config;
use state::AppState;
use tauri::{Manager, WindowEvent};

pub fn run() {
    tauri::Builder::default()
        // 单实例必须最先注册。第二个实例启动时 → 把主窗口唤出来。
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            tray::open_main_window(app);
        }))
        .plugin(
            tauri_plugin_log::Builder::new()
                .targets([
                    tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::Stdout),
                    tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::LogDir {
                        file_name: Some("sms-pop".to_string()),
                    }),
                ])
                .level(log::LevelFilter::Info)
                .timezone_strategy(tauri_plugin_log::TimezoneStrategy::UseLocal)
                .build(),
        )
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(updater::UpdateState::default())
        .setup(|app| {
            let first_run = !paths::config_path(app.handle()).exists();
            if first_run {
                #[cfg(windows)]
                let _ = Config::write_default_config(paths::config_path(app.handle()));
                #[cfg(target_os = "macos")]
                Config::initial_for_platform(true).save(paths::config_path(app.handle()))?;
            }

            app.manage(AppState::new(app.handle()));
            #[cfg(target_os = "macos")]
            shortcut::setup(app.handle())?;
            app.manage(http_ingress::HttpIngress::load(app.handle()));
            http_ingress::start(app.handle().clone());

            tray::setup(app.handle())?;
            #[cfg(windows)]
            link_worker::spawn(app.handle().clone());
            updater::start(app.handle().clone());

            // 主窗口关到托盘，不退出
            if let Some(main_window) = app.get_webview_window("main") {
                let window_for_event = main_window.clone();
                main_window.on_window_event(move |event| {
                    if let WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        let _ = window_for_event.hide();
                    }
                });

                // Mac 打开应用时显示设置；Windows 保留首次运行引导行为。
                if first_run || cfg!(target_os = "macos") {
                    let _ = main_window.show();
                    let _ = main_window.set_focus();
                }
            }

            info!("SmsPop 已启动");

            Ok(())
        })
        .invoke_handler({
            #[cfg(target_os = "macos")]
            {
                tauri::generate_handler![
                    platform::get_platform_capabilities,
                    commands::get_config,
                    http_ingress::get_http_status,
                    http_ingress::list_http_addresses,
                    http_ingress::open_firewall_rules,
                    http_ingress::configure_http,
                    http_ingress::reset_http_token,
                    http_ingress::set_http_copy,
                    updater::get_update_status,
                    updater::check_update,
                    updater::download_update,
                    updater::install_update,
                    commands::save_config,
                    commands::get_settings_meta,
                    commands::set_autostart,
                    commands::open_config_dir,
                    commands::open_logs_dir,
                    commands::open_project_link,
                    commands::open_bluetooth_settings,
                    commands::get_link_state,
                    commands::list_devices,
                    commands::set_preferred_device,
                    commands::set_device_enabled,
                    commands::forget_device,
                    commands::send_test_notification,
                    commands::get_toast_payload,
                    commands::toast_click,
                    commands::toast_open_app,
                    commands::test_app_rule,
                    commands::validate_app_rule,
                    commands::toast_close,
                    commands::toast_resize,
                    commands::get_caret_offer,
                    commands::caret_layout,
                    commands::caret_insert,
                    commands::caret_hide,
                    commands::open_main_window,
                    shortcut::get_input_status,
                    shortcut::request_input_access,
                    shortcut::set_input_shortcut,
                ]
            }
            #[cfg(not(target_os = "macos"))]
            {
                tauri::generate_handler![
                    platform::get_platform_capabilities,
                    commands::get_config,
                    http_ingress::get_http_status,
                    http_ingress::list_http_addresses,
                    http_ingress::open_firewall_rules,
                    http_ingress::configure_http,
                    http_ingress::reset_http_token,
                    http_ingress::set_http_copy,
                    updater::get_update_status,
                    updater::check_update,
                    updater::download_update,
                    updater::install_update,
                    commands::save_config,
                    commands::get_settings_meta,
                    commands::set_autostart,
                    commands::open_config_dir,
                    commands::open_logs_dir,
                    commands::open_project_link,
                    commands::open_bluetooth_settings,
                    commands::get_link_state,
                    commands::list_devices,
                    commands::set_preferred_device,
                    commands::set_device_enabled,
                    commands::forget_device,
                    commands::send_test_notification,
                    commands::get_toast_payload,
                    commands::toast_click,
                    commands::toast_open_app,
                    commands::test_app_rule,
                    commands::validate_app_rule,
                    commands::toast_close,
                    commands::toast_resize,
                    commands::get_caret_offer,
                    commands::caret_layout,
                    commands::caret_insert,
                    commands::caret_hide,
                    commands::open_main_window,
                ]
            }
        })
        .build(tauri::generate_context!())
        .expect("构建 Tauri 应用失败")
        .run(|app, event| {
            #[cfg(target_os = "macos")]
            if let tauri::RunEvent::Reopen { .. } = event {
                tray::open_main_window(app);
            }
            if let tauri::RunEvent::Exit = event {
                http_ingress::stop(app);
                app.state::<AppState>()
                    .link_stop
                    .store(true, std::sync::atomic::Ordering::Relaxed);
            }
        });
}
