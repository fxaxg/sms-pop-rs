//! 更新任务由后端持有，设置窗口隐藏或页面卸载不影响下载。
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_updater::{Update, UpdaterExt};

#[derive(Clone, Default, Serialize)]
pub struct UpdateStatus {
    pub revision: u64,
    pub phase: String,
    pub version: Option<String>,
    pub notes: Option<String>,
    pub downloaded: u64,
    pub total: Option<u64>,
    pub error: Option<String>,
}

pub struct UpdateState {
    status: Mutex<UpdateStatus>,
    operation: tokio::sync::Mutex<()>,
    update: Mutex<Option<Update>>,
    bytes: Mutex<Option<Vec<u8>>>,
}

impl Default for UpdateState {
    fn default() -> Self {
        Self {
            status: Mutex::new(UpdateStatus {
                phase: if cfg!(debug_assertions) {
                    "disabled"
                } else {
                    "idle"
                }
                .into(),
                ..Default::default()
            }),
            operation: tokio::sync::Mutex::new(()),
            update: Mutex::new(None),
            bytes: Mutex::new(None),
        }
    }
}

fn publish(app: &AppHandle, mutate: impl FnOnce(&mut UpdateStatus)) {
    let state = app.state::<UpdateState>();
    let mut status = state.status.lock().unwrap();
    mutate(&mut status);
    status.revision += 1;
    // 持锁广播，保证事件与查询的 revision 顺序一致。
    let _ = app.emit_to("main", "update-status", status.clone());
}

fn failure(app: &AppHandle, phase: &str, error: String) -> String {
    log::warn!("更新 {phase} 失败：{error}");
    publish(app, |s| {
        s.phase = phase.into();
        s.error = Some(error.clone());
    });
    error
}

fn enabled() -> Result<(), String> {
    if cfg!(debug_assertions) {
        Err("Updates are disabled in development builds".into())
    } else {
        Ok(())
    }
}

fn main_only(window: &tauri::WebviewWindow) -> Result<(), String> {
    if window.label() == "main" {
        Ok(())
    } else {
        Err("Updates are only available in the settings window".into())
    }
}

#[tauri::command]
pub fn get_update_status(
    window: tauri::WebviewWindow,
    state: tauri::State<'_, UpdateState>,
) -> Result<UpdateStatus, String> {
    main_only(&window)?;
    Ok(state.status.lock().unwrap().clone())
}

pub async fn check(app: AppHandle) -> Result<(), String> {
    enabled()?;
    let state = app.state::<UpdateState>();
    let _operation = state
        .operation
        .try_lock()
        .map_err(|_| "Update operation already in progress")?;
    if state.bytes.lock().unwrap().is_some() {
        return Ok(());
    }
    publish(&app, |s| {
        s.phase = "checking".into();
        s.error = None;
    });
    let result = async {
        app.updater_builder()
            .timeout(Duration::from_secs(20))
            .build()?
            .check()
            .await
    }
    .await;
    match result {
        Ok(update) => {
            publish(&app, |s| {
                s.phase = if update.is_some() {
                    "available"
                } else {
                    "latest"
                }
                .into();
                s.version = update.as_ref().map(|u| u.version.clone());
                s.notes = update.as_ref().and_then(|u| u.body.clone());
                s.downloaded = 0;
                s.total = None;
            });
            *state.update.lock().unwrap() = update;
            Ok(())
        }
        Err(e) => Err(failure(&app, "check_error", e.to_string())),
    }
}

#[tauri::command]
pub async fn check_update(window: tauri::WebviewWindow, app: AppHandle) -> Result<(), String> {
    main_only(&window)?;
    check(app).await
}

#[tauri::command]
pub async fn download_update(window: tauri::WebviewWindow, app: AppHandle) -> Result<(), String> {
    main_only(&window)?;
    enabled()?;
    let state = app.state::<UpdateState>();
    let _operation = state
        .operation
        .try_lock()
        .map_err(|_| "Update operation already in progress")?;
    if state.bytes.lock().unwrap().is_some() {
        return Ok(());
    }
    let mut update = state
        .update
        .lock()
        .unwrap()
        .clone()
        .ok_or("Check for updates first")?;
    update.timeout = Some(Duration::from_secs(30 * 60));
    publish(&app, |s| {
        s.phase = "downloading".into();
        s.downloaded = 0;
        s.total = None;
        s.error = None;
    });
    let mut downloaded = 0;
    let mut last = Instant::now();
    let result = update
        .download(
            |chunk, total| {
                downloaded += chunk as u64;
                if last.elapsed() >= Duration::from_millis(150) {
                    publish(&app, |s| {
                        s.downloaded = downloaded;
                        s.total = total;
                    });
                    last = Instant::now();
                }
            },
            || {},
        )
        .await;
    match result {
        Ok(bytes) => {
            let length = bytes.len() as u64;
            *state.bytes.lock().unwrap() = Some(bytes);
            publish(&app, |s| {
                s.phase = "ready".into();
                s.downloaded = length;
                s.total = Some(length);
            });
            Ok(())
        }
        Err(e) => Err(failure(&app, "download_error", e.to_string())),
    }
}

#[tauri::command]
pub async fn install_update(window: tauri::WebviewWindow, app: AppHandle) -> Result<(), String> {
    main_only(&window)?;
    enabled()?;
    let state = app.state::<UpdateState>();
    let _operation = state
        .operation
        .try_lock()
        .map_err(|_| "Update operation already in progress")?;
    let update = state
        .update
        .lock()
        .unwrap()
        .clone()
        .ok_or("Check for updates first")?;
    let bytes = state
        .bytes
        .lock()
        .unwrap()
        .take()
        .ok_or("Download the update first")?;
    publish(&app, |s| {
        s.phase = "installing".into();
        s.error = None;
    });
    // 安装涉及磁盘和启动安装器，不占用异步运行时线程。
    let result = tauri::async_runtime::spawn_blocking(move || {
        // Windows 插件在成功启动安装器后直接退出进程；不要提前停止 BLE，
        // 否则启动安装器失败后通知链路无法继续工作。
        let result = update.install(&bytes);
        (result, bytes)
    })
    .await;
    match result {
        Ok((Ok(()), _)) => Ok(()),
        Ok((Err(e), bytes)) => {
            *state.bytes.lock().unwrap() = Some(bytes);
            Err(failure(&app, "install_error", e.to_string()))
        }
        Err(e) => Err(failure(&app, "download_error", e.to_string())),
    }
}

pub fn start(app: AppHandle) {
    if cfg!(debug_assertions) {
        return;
    }
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_secs(15)).await;
        let _ = check(app).await;
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn development_builds_disable_updates_and_have_no_download() {
        let state = UpdateState::default();
        assert_eq!(
            state.status.lock().unwrap().phase,
            if cfg!(debug_assertions) {
                "disabled"
            } else {
                "idle"
            }
        );
        assert!(state.update.lock().unwrap().is_none());
        assert!(state.bytes.lock().unwrap().is_none());
        assert_eq!(enabled().is_err(), cfg!(debug_assertions));
    }

    #[test]
    fn unknown_size_is_serialized_as_null_not_a_fake_percentage() {
        let status = UpdateStatus {
            phase: "downloading".into(),
            downloaded: 2048,
            ..Default::default()
        };
        let value = serde_json::to_value(status).unwrap();
        assert!(value["total"].is_null());
        assert_eq!(value["downloaded"], 2048);
    }
}
