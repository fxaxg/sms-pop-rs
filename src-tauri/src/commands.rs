//! 前端可调用的命令。
//!
//! 命名规则：名词_动词。凡是会改状态的都返回 `Result<_, String>`，
//! 错误信息直接给人看。

use log::{info, warn};
use serde::Serialize;
use smspop_core::ancs::Attributes;
use smspop_core::config::Config;
use smspop_core::model::PhoneNotification;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_clipboard_manager::ClipboardExt;

use crate::state::AppState;
use crate::types::{CaretLayout, LinkStatePayload, ToastPayload};
use crate::{caret, link_worker, paths, popups, tray};

// ── 设置窗口 ──────────────────────────────────────────────────

#[tauri::command]
pub fn get_config(state: State<'_, AppState>) -> Config {
    state.config()
}

#[tauri::command]
pub fn save_config(app: AppHandle, config: Config) -> Result<(), String> {
    let path = paths::config_path(&app);

    config
        .save(&path)
        .map_err(|error| format!("保存配置失败：{error}"))?;

    app.state::<AppState>().apply_config(config);
    info!("配置已保存");

    Ok(())
}

/// 设置界面要的元信息（版本、路径、自启状态）。
#[derive(Debug, Clone, Serialize)]
pub struct SettingsMeta {
    pub version: String,
    pub config_path: String,
    pub log_dir: String,
    pub autostart_enabled: bool,
}

#[tauri::command]
pub fn get_settings_meta(app: AppHandle) -> SettingsMeta {
    use tauri_plugin_autostart::ManagerExt;

    SettingsMeta {
        version: app.package_info().version.to_string(),
        config_path: paths::config_path(&app).display().to_string(),
        log_dir: paths::log_dir(&app).display().to_string(),
        autostart_enabled: app.autolaunch().is_enabled().unwrap_or(false),
    }
}

#[tauri::command]
pub fn set_autostart(app: AppHandle, enabled: bool) -> Result<bool, String> {
    use tauri_plugin_autostart::ManagerExt;

    let autostart = app.autolaunch();

    let result = if enabled {
        autostart.enable()
    } else {
        autostart.disable()
    };

    result.map_err(|error| format!("设置开机自启失败：{error}"))?;

    autostart
        .is_enabled()
        .map_err(|error| format!("读取开机自启状态失败：{error}"))
}

#[tauri::command]
pub fn open_config_dir(app: AppHandle) -> Result<(), String> {
    open_in_explorer(&paths::config_path(&app))
}

#[tauri::command]
pub fn open_logs_dir(app: AppHandle) -> Result<(), String> {
    open_in_explorer(&paths::log_dir(&app))
}

#[tauri::command]
pub fn open_bluetooth_settings() -> Result<(), String> {
    std::process::Command::new("cmd")
        .args(["/c", "start", "ms-settings:bluetooth"])
        .spawn()
        .map_err(|error| format!("打开蓝牙设置失败：{error}"))?;

    Ok(())
}

fn open_in_explorer(path: &std::path::Path) -> Result<(), String> {
    std::process::Command::new("explorer")
        .arg(path)
        .spawn()
        .map_err(|error| format!("打开资源管理器失败：{error}"))?;

    Ok(())
}

// ── 链路 ──────────────────────────────────────────────────────

#[tauri::command]
pub fn get_link_state(state: State<'_, AppState>) -> LinkStatePayload {
    let guard = state
        .link_state
        .read()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let (state, detail) = guard.clone();

    LinkStatePayload {
        state: crate::types::state_key(state).to_string(),
        label: state.describe().to_string(),
        detail,
        ready: state.is_ready(),
    }
}

/// 造一条测试通知，走和真实通知完全相同的路径（去重除外——每次换新 uid）。
///
/// 延迟 4 秒再发：留出时间让用户把光标点进一个输入框，顺便验证光标候选条。
/// （从「验证纪律」角度，这一条和真实通知走的是完全相同的代码路径。）
#[tauri::command]
pub fn send_test_notification(app: AppHandle) {
    send_test_notification_from(app);
}

/// 托盘菜单也用同一个入口（托盘回调拿不到 `State`，只能拿 `AppHandle`）。
pub fn send_test_notification_from(app: AppHandle) {
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(4));

        let uid = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.subsec_nanos())
            .unwrap_or(1);

        let notification = PhoneNotification::from_ancs(
            "test-device",
            Some("测试".to_string()),
            Attributes {
                uid,
                app_identifier: Some("com.apple.MobileSMS".to_string()),
                title: Some("SmsPop".to_string()),
                message: Some("【SmsPop】这是一条测试通知，验证码：868740。".to_string()),
                ..Default::default()
            },
        );

        info!("发送测试通知");
        link_worker::dispatch_notification(&app, notification);
    });
}

// ── toast 窗口 ────────────────────────────────────────────────

/// toast 前端加载完成后主动来取自己的负载。
#[tauri::command]
pub fn get_toast_payload(
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
) -> Option<ToastPayload> {
    state
        .pending_toasts
        .lock()
        .unwrap()
        .get(window.label())
        .cloned()
}

/// 用户点了 toast：有验证码就复制，返回复制了什么。
#[tauri::command]
pub fn toast_click(app: AppHandle, label: String, state: State<'_, AppState>) -> Option<String> {
    let code = state
        .pending_toasts
        .lock()
        .unwrap()
        .get(&label)
        .and_then(|payload| payload.code.clone());

    if let Some(code) = &code {
        if let Err(error) = app.clipboard().write_text(code.clone()) {
            warn!("复制验证码失败：{error}");
            return None;
        }
    }

    code
}

#[tauri::command]
pub fn toast_close(app: AppHandle, label: String) {
    popups::close_toast(&app, &label);
}

// ── 候选条窗口 ────────────────────────────────────────────────

/// caret 前端加载完成后主动来取当前提议。
#[tauri::command]
pub fn get_caret_offer(state: State<'_, AppState>) -> Option<(u64, String, u32)> {
    state
        .caret_offer
        .lock()
        .unwrap()
        .as_ref()
        .map(|offer| (offer.generation, offer.code.clone(), offer.duration_secs))
}

#[tauri::command]
pub fn caret_layout(app: AppHandle, layout: CaretLayout) {
    caret::layout(&app, layout);
}

#[tauri::command]
pub fn caret_insert(app: AppHandle, generation: u64) -> Option<(bool, String)> {
    caret::insert(&app, generation)
}

#[tauri::command]
pub fn caret_hide(app: AppHandle) {
    caret::hide(&app);
}

// ── 主窗口行为 ────────────────────────────────────────────────

#[tauri::command]
pub fn open_main_window(app: AppHandle) {
    tray::open_main_window(&app);
}
