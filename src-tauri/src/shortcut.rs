use crate::{macos_input, state::AppState};
use serde::Serialize;
use smspop_core::input_action::PressLatch;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Mutex,
};
use std::time::Instant;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

pub const DEFAULT_SHORTCUT: &str = "CommandOrControl+Shift+V";
#[derive(Default)]
pub struct ShortcutStateData {
    registered: Mutex<Option<String>>,
    error: Mutex<Option<String>>,
    latch: Mutex<PressLatch>,
    busy: AtomicBool,
}
#[derive(Serialize)]
pub struct InputStatus {
    authorized: bool,
    registered_shortcut: Option<String>,
    last_error: Option<String>,
}
fn main_only(window: &tauri::WebviewWindow) -> Result<(), String> {
    if window.label() == "main" {
        Ok(())
    } else {
        Err("Settings window required".into())
    }
}
fn publish(app: &AppHandle, result: Result<(), String>) {
    let message = result.err();
    *app.state::<ShortcutStateData>().error.lock().unwrap() = message.clone();
    let text = message.unwrap_or_else(|| "已发送填入指令；若目标未接受，请手动复制".into());
    let _ = app.emit("input-result", text.clone());
    let state = app.state::<AppState>();
    let notification = feedback(
        text,
        &mut state.otp_candidate.lock().unwrap(),
        Instant::now(),
    );
    crate::popups::show(app, &notification);
}
fn feedback(
    text: String,
    candidate: &mut smspop_core::otp_candidate::OtpCandidateStore,
    now: Instant,
) -> smspop_core::model::PhoneNotification {
    smspop_core::model::PhoneNotification {
        device_id: "local-feedback".into(),
        device_name: Some("SmsPop".into()),
        uid: 0,
        app_identifier: None,
        title: Some("SmsPop · 填入状态".into()),
        subtitle: None,
        message: Some(text),
        code: candidate.current(now).map(str::to_owned),
        received_at: std::time::SystemTime::now(),
    }
}

pub fn configure(app: &AppHandle, accelerator: &str) -> Result<(), String> {
    let shortcut: Shortcut = accelerator
        .parse()
        .map_err(|_| "快捷键格式不正确，如 CommandOrControl+Shift+V")?;
    // Plain characters must never become global shortcuts.
    if shortcut.mods.is_empty() {
        return Err("快捷键必须包含 Command、Control、Alt 或 Shift".into());
    }
    let state = app.state::<ShortcutStateData>();
    let old = state.registered.lock().unwrap().clone();
    if old.as_deref() == Some(accelerator) {
        return Ok(());
    }
    app.global_shortcut()
        .on_shortcut(shortcut, |app, _, event| {
            let state = app.state::<ShortcutStateData>();
            let fire = state
                .latch
                .lock()
                .unwrap()
                .event(event.state() == ShortcutState::Pressed);
            if !fire || state.busy.swap(true, Ordering::SeqCst) {
                return;
            }
            let handle = app.clone();
            tauri::async_runtime::spawn_blocking(move || {
                let result = insert_latest(&handle);
                publish(&handle, result);
                handle
                    .state::<ShortcutStateData>()
                    .busy
                    .store(false, Ordering::SeqCst);
            });
        })
        .map_err(|_| "快捷键注册失败，可能已被其他应用占用")?;
    if let Some(old) = old {
        if let Err(e) = app.global_shortcut().unregister(old.as_str()) {
            let _ = app.global_shortcut().unregister(shortcut);
            return Err(format!("无法移除旧快捷键：{e}"));
        }
    }
    *state.registered.lock().unwrap() = Some(accelerator.into());
    *state.latch.lock().unwrap() = PressLatch::default();
    *state.error.lock().unwrap() = None;
    Ok(())
}
pub fn insert_latest(app: &AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    if !state.config().otp.enabled {
        return Err("验证码功能已关闭".into());
    }
    let code = state
        .otp_candidate
        .lock()
        .unwrap()
        .current(Instant::now())
        .map(str::to_owned)
        .ok_or("没有有效验证码，请重新获取或手动复制")?;
    macos_input::insert(&code, || {
        state.config().otp.enabled
            && state.otp_candidate.lock().unwrap().current(Instant::now()) == Some(code.as_str())
    })
}
pub fn setup(app: &AppHandle) -> Result<(), String> {
    app.manage(ShortcutStateData::default());
    app.plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .map_err(|e| e.to_string())?;
    let path = crate::paths::data_dir(app).join("macos-shortcut.json");
    let shortcut = std::fs::read(&path)
        .ok()
        .and_then(|b| serde_json::from_slice::<String>(&b).ok())
        .unwrap_or_else(|| DEFAULT_SHORTCUT.into());
    if let Err(e) = configure(app, &shortcut) {
        publish(app, Err(e));
    }
    Ok(())
}
#[tauri::command]
pub fn get_input_status(
    window: tauri::WebviewWindow,
    app: AppHandle,
) -> Result<InputStatus, String> {
    main_only(&window)?;
    let state = app.state::<ShortcutStateData>();
    let registered_shortcut = state.registered.lock().unwrap().clone();
    let last_error = state.error.lock().unwrap().clone();
    Ok(InputStatus {
        authorized: macos_input::is_trusted(),
        registered_shortcut,
        last_error,
    })
}
#[tauri::command]
pub fn request_input_access(window: tauri::WebviewWindow) -> Result<bool, String> {
    main_only(&window)?;
    macos_input::request_access()
}
#[tauri::command]
pub fn set_input_shortcut(
    window: tauri::WebviewWindow,
    app: AppHandle,
    accelerator: String,
) -> Result<(), String> {
    main_only(&window)?;
    let old = app
        .state::<ShortcutStateData>()
        .registered
        .lock()
        .unwrap()
        .clone();
    configure(&app, &accelerator)?;
    let path = crate::paths::data_dir(&app).join("macos-shortcut.json");
    let result = std::fs::write(
        path.with_extension("tmp"),
        serde_json::to_vec(&accelerator).map_err(|e| e.to_string())?,
    )
    .and_then(|_| std::fs::rename(path.with_extension("tmp"), &path));
    if result.is_err() {
        if let Some(old) = old {
            if let Err(e) = configure(&app, &old) {
                publish(&app, Err(e));
            }
        } else {
            let _ = app.global_shortcut().unregister(accelerator.as_str());
            *app.state::<ShortcutStateData>().registered.lock().unwrap() = None;
        }
        return Err("快捷键保存失败，已尝试恢复原快捷键".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn failed_insertion_feedback_exposes_only_unexpired_candidate() {
        let mut candidate = smspop_core::otp_candidate::OtpCandidateStore::default();
        let now = Instant::now();
        candidate.offer("123456".into(), now);
        assert_eq!(
            feedback("Not editable".into(), &mut candidate, now)
                .code
                .as_deref(),
            Some("123456")
        );
        assert_eq!(
            feedback(
                "Expired".into(),
                &mut candidate,
                now + std::time::Duration::from_secs(120)
            )
            .code,
            None
        );
    }
}
