//! SmsPop 主程序。
//!
//! 组装：插件 → 全局状态 → 托盘 → 蓝牙链路线程 → 命令。

mod caret;
mod commands;
mod devices;
mod link_worker;
mod paths;
mod popups;
mod state;
mod tray;
mod types;

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
        .setup(|app| {
            let first_run = !paths::config_path(app.handle()).exists();
            if first_run {
                let _ = Config::write_default_config(paths::config_path(app.handle()));
            }

            app.manage(AppState::new(app.handle()));

            tray::setup(app.handle())?;
            link_worker::spawn(app.handle().clone());

            // 主窗口关到托盘，不退出
            if let Some(main_window) = app.get_webview_window("main") {
                let window_for_event = main_window.clone();
                main_window.on_window_event(move |event| {
                    if let WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        let _ = window_for_event.hide();
                    }
                });

                // 首次运行 → 打开主窗口（默认落在「连接引导」页）
                if first_run {
                    let _ = main_window.show();
                    let _ = main_window.set_focus();
                }
            }

            info!("SmsPop 已启动");

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_config,
            commands::save_config,
            commands::get_settings_meta,
            commands::set_autostart,
            commands::open_config_dir,
            commands::open_logs_dir,
            commands::open_bluetooth_settings,
            commands::get_link_state,
            commands::list_devices,
            commands::set_preferred_device,
            commands::set_device_enabled,
            commands::forget_device,
            commands::send_test_notification,
            commands::get_toast_payload,
            commands::toast_click,
            commands::toast_close,
            commands::toast_resize,
            commands::get_caret_offer,
            commands::caret_layout,
            commands::caret_insert,
            commands::caret_hide,
            commands::open_main_window,
        ])
        .build(tauri::generate_context!())
        .expect("构建 Tauri 应用失败")
        .run(|app, event| {
            if let tauri::RunEvent::Exit = event {
                app.state::<AppState>()
                    .link_stop
                    .store(true, std::sync::atomic::Ordering::Relaxed);
            }
        });
}
