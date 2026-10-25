//! 更新任务由后端持有，设置窗口隐藏或页面卸载不影响下载。
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_updater::{Update, UpdaterExt};
use tokio::sync::watch;

/// Only checks are scheduled. A generation invalidates timers superseded by manual actions.
#[derive(Clone, Default)]
struct CheckSchedule {
    generation: u64,
    failures: u32,
    due: Option<tokio::time::Instant>,
}

impl CheckSchedule {
    fn cancel(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        self.due = None;
    }

    fn success(&mut self) {
        self.cancel();
        self.failures = 0;
    }

    fn failed(&mut self, now: tokio::time::Instant) -> Duration {
        self.cancel();
        self.failures = self.failures.saturating_add(1);
        let seconds = match self.failures {
            1 => 30,
            2 => 60,
            3 => 120,
            4 => 300,
            _ => 900,
        };
        let delay = Duration::from_secs(seconds);
        self.due = Some(now + delay);
        delay
    }
}

async fn supervise<F, Fut>(mut schedule: watch::Receiver<CheckSchedule>, mut check: F)
where
    F: FnMut(u64) -> Fut,
    Fut: std::future::Future<Output = ()>,
{
    loop {
        let current = schedule.borrow_and_update().clone();
        if let Some(due) = current.due {
            tokio::select! {
                result = schedule.changed() => { if result.is_err() { return; } }
                () = tokio::time::sleep_until(due) => check(current.generation).await,
            }
        } else if schedule.changed().await.is_err() {
            return;
        }
    }
}

#[derive(Clone, Default, Serialize)]
pub struct UpdateStatus {
    pub revision: u64,
    pub phase: String,
    pub version: Option<String>,
    pub notes: Option<String>,
    pub downloaded: u64,
    pub total: Option<u64>,
    pub error: Option<String>,
    pub retry_attempt: u32,
    pub next_retry_at: Option<u64>,
}

pub struct UpdateState {
    status: Mutex<UpdateStatus>,
    operation: tokio::sync::Mutex<()>,
    update: Mutex<Option<Update>>,
    bytes: Mutex<Option<Vec<u8>>>,
    schedule: watch::Sender<CheckSchedule>,
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
            schedule: watch::channel(CheckSchedule::default()).0,
        }
    }
}

impl UpdateState {
    async fn begin_check(
        &self,
        generation: Option<u64>,
    ) -> Result<Option<tokio::sync::MutexGuard<'_, ()>>, String> {
        // A timer that was already waiting for another operation must be revalidated.
        let operation = if let Some(generation) = generation {
            let operation = self.operation.lock().await;
            if self.schedule.borrow().generation != generation {
                return Ok(None);
            }
            operation
        } else {
            self.operation
                .try_lock()
                .map_err(|_| "Update operation already in progress")?
        };
        self.schedule.send_modify(CheckSchedule::cancel);
        Ok(Some(operation))
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

async fn check(app: AppHandle, scheduled_generation: Option<u64>) -> Result<(), String> {
    enabled()?;
    let state = app.state::<UpdateState>();
    let Some(_operation) = state.begin_check(scheduled_generation).await? else {
        return Ok(());
    };
    if state.bytes.lock().unwrap().is_some() {
        state.schedule.send_modify(CheckSchedule::success);
        publish(&app, |s| {
            s.retry_attempt = 0;
            s.next_retry_at = None;
        });
        return Ok(());
    }
    publish(&app, |s| {
        s.phase = "checking".into();
        s.error = None;
        s.next_retry_at = None;
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
            // Store the update before publishing so a download can use this exact version.
            *state.update.lock().unwrap() = update.clone();
            state.schedule.send_modify(CheckSchedule::success);
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
                s.retry_attempt = 0;
                s.next_retry_at = None;
            });
            Ok(())
        }
        Err(e) => {
            let error = e.to_string();
            let mut delay = Duration::ZERO;
            state.schedule.send_modify(|schedule| {
                delay = schedule.failed(tokio::time::Instant::now());
            });
            let attempt = state.schedule.borrow().failures;
            let next_retry_at = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .saturating_add(delay)
                .as_millis() as u64;
            publish(&app, |s| {
                s.phase = "check_error".into();
                s.error = Some(error.clone());
                s.retry_attempt = attempt;
                s.next_retry_at = Some(next_retry_at);
            });
            log::warn!("更新 check_error 失败：{error}");
            Err(error)
        }
    }
}

#[tauri::command]
pub async fn check_update(window: tauri::WebviewWindow, app: AppHandle) -> Result<(), String> {
    main_only(&window)?;
    check(app, None).await
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
    // A previously found version can still be downloaded after a failed recheck.
    state.schedule.send_modify(CheckSchedule::success);
    update.timeout = Some(Duration::from_secs(30 * 60));
    publish(&app, |s| {
        s.phase = "downloading".into();
        s.downloaded = 0;
        s.total = None;
        s.error = None;
        s.retry_attempt = 0;
        s.next_retry_at = None;
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
    let state = app.state::<UpdateState>();
    state.schedule.send_modify(|schedule| {
        schedule.cancel();
        schedule.due = Some(tokio::time::Instant::now() + Duration::from_secs(15));
    });
    let schedule = state.schedule.subscribe();
    tauri::async_runtime::spawn(supervise(schedule, move |generation| {
        let app = app.clone();
        async move {
            let _ = check(app, Some(generation)).await;
        }
    }));
}

fn validate_notes_link(url: &str) -> Result<(), String> {
    let parsed = tauri::Url::parse(url).map_err(|_| "Invalid update notes URL")?;
    if parsed.scheme() != "https"
        || parsed.host_str().is_none()
        || !parsed.username().is_empty()
        || parsed.password().is_some()
    {
        return Err("Update notes links must be HTTPS URLs without credentials".into());
    }
    smspop_core::app_rules::validate_url(url)
}

#[tauri::command]
pub fn open_update_link(window: tauri::WebviewWindow, url: String) -> Result<(), String> {
    main_only(&window)?;
    validate_notes_link(&url)?;
    crate::app_launcher::open(&smspop_core::app_rules::AppRule {
        app_id: "smspop.updater".into(),
        name: "Update notes".into(),
        enabled: true,
        target: url,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_https_notes_links_without_credentials_are_allowed() {
        assert!(validate_notes_link("https://github.com/fxaxg/sms-pop-rs/releases").is_ok());
        for url in [
            "http://example.com",
            "javascript:alert(1)",
            "file:///C:/test",
            "weixin://",
            "https://user:password@example.com",
            "/relative",
            "https://",
            "https://example.com/\n",
        ] {
            assert!(validate_notes_link(url).is_err(), "{url:?}");
        }
    }

    #[tokio::test(start_paused = true)]
    async fn backoff_is_measured_from_completion_and_capped_then_resets() {
        let mut schedule = CheckSchedule::default();
        for seconds in [30, 60, 120, 300, 900, 900, 900] {
            // Simulate a slow failed request before scheduling the next one.
            tokio::time::advance(Duration::from_secs(20)).await;
            let completion = tokio::time::Instant::now();
            assert_eq!(schedule.failed(completion), Duration::from_secs(seconds));
            assert_eq!(
                schedule.due,
                Some(completion + Duration::from_secs(seconds))
            );
        }
        schedule.success();
        assert!(schedule.due.is_none());
        assert_eq!(schedule.failures, 0);
        assert_eq!(
            schedule.failed(tokio::time::Instant::now()),
            Duration::from_secs(30)
        );
    }

    #[tokio::test(start_paused = true)]
    async fn supervisor_recovers_after_failures_and_stops_after_success() {
        use std::sync::{
            atomic::{AtomicUsize, Ordering},
            Arc,
        };
        let (tx, rx) = watch::channel(CheckSchedule {
            due: Some(tokio::time::Instant::now() + Duration::from_secs(15)),
            ..Default::default()
        });
        let calls = Arc::new(AtomicUsize::new(0));
        let output = calls.clone();
        let sender = tx.clone();
        let worker = tokio::spawn(supervise(rx, move |_| {
            let count = output.fetch_add(1, Ordering::SeqCst);
            let sender = sender.clone();
            async move {
                if count < 2 {
                    sender.send_modify(|s| {
                        s.failed(tokio::time::Instant::now());
                    });
                } else {
                    sender.send_modify(CheckSchedule::success);
                }
            }
        }));
        tokio::task::yield_now().await;
        for (seconds, expected) in [(14, 0), (1, 1), (29, 1), (1, 2), (59, 2), (1, 3), (3600, 3)] {
            tokio::time::advance(Duration::from_secs(seconds)).await;
            tokio::task::yield_now().await;
            assert_eq!(calls.load(Ordering::SeqCst), expected);
        }
        assert_eq!(tx.borrow().failures, 0);
        assert!(tx.borrow().due.is_none());
        worker.abort();
    }

    #[tokio::test(start_paused = true)]
    async fn manual_check_invalidates_queued_timer_without_losing_download() {
        use std::sync::Arc;
        let state = Arc::new(UpdateState::default());
        state.schedule.send_modify(|s| {
            s.failed(tokio::time::Instant::now());
        });
        let generation = state.schedule.borrow().generation;
        let manual = state.begin_check(None).await.unwrap().unwrap();
        // Concurrent manual requests cannot duplicate the request or increase the backoff.
        assert!(state.begin_check(None).await.is_err());
        assert_eq!(state.schedule.borrow().failures, 1);
        let queued = state.clone();
        let waiter = tokio::spawn(async move {
            queued
                .begin_check(Some(generation))
                .await
                .unwrap()
                .is_none()
        });
        tokio::task::yield_now().await;
        *state.bytes.lock().unwrap() = Some(vec![1, 2, 3]);
        state.status.lock().unwrap().phase = "ready".into();
        state.schedule.send_modify(CheckSchedule::success);
        drop(manual);
        assert!(waiter.await.unwrap());
        assert_eq!(*state.bytes.lock().unwrap(), Some(vec![1, 2, 3]));
        assert_eq!(state.status.lock().unwrap().phase, "ready");
        assert!(state.schedule.borrow().due.is_none());
    }

    #[tokio::test(start_paused = true)]
    async fn cancelled_retry_never_fires_at_its_old_deadline() {
        use std::sync::{
            atomic::{AtomicUsize, Ordering},
            Arc,
        };
        let mut initial = CheckSchedule::default();
        initial.failed(tokio::time::Instant::now());
        let (tx, rx) = watch::channel(initial);
        let calls = Arc::new(AtomicUsize::new(0));
        let output = calls.clone();
        let worker = tokio::spawn(supervise(rx, move |_| {
            output.fetch_add(1, Ordering::SeqCst);
            async {}
        }));
        tokio::task::yield_now().await;
        tokio::time::advance(Duration::from_secs(10)).await;
        tx.send_modify(CheckSchedule::success);
        tokio::task::yield_now().await;
        tokio::time::advance(Duration::from_secs(3600)).await;
        tokio::task::yield_now().await;
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        worker.abort();
    }

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
