//! 右下角通知弹窗（toast）的窗口管理。
//!
//! 每条通知是一个独立的小窗口：无边框、透明、置顶、不抢焦点。
//! 堆叠在屏幕右下角，新的在最下面，超上限先关最旧的。

use std::sync::atomic::Ordering;
use std::time::Duration;

use log::{info, warn};
use smspop_core::model::PhoneNotification;
use tauri::{
    AppHandle, Manager, PhysicalPosition, Position, WebviewUrl, WebviewWindowBuilder, WindowEvent,
};

use crate::state::AppState;
use crate::types::ToastPayload;

/// toast 尺寸与间距（DIP；乘显示器缩放后换成物理像素）。
const TOAST_HEIGHT: f64 = 96.0;
const MARGIN: f64 = 16.0;
const GAP: f64 = 8.0;

/// 弹一条通知。
pub fn show(app: &AppHandle, notification: &PhoneNotification) {
    let state = app.state::<AppState>();
    let config = state.config();
    let popup = &config.notifications.popup;

    // 超上限先关最旧的
    {
        let open: Vec<String> = state.toasts.lock().unwrap().iter().cloned().collect();
        let excess = open
            .len()
            .saturating_add(1)
            .saturating_sub(popup.max_visible.max(1));
        for label in open.into_iter().take(excess) {
            close_toast(app, &label);
        }
    }

    let id = state.toast_seq.fetch_add(1, Ordering::Relaxed);
    let label = format!("toast-{id}");

    let payload = ToastPayload {
        origin: notification.origin(),
        body: notification.summary(),
        code: notification.code.clone(),
        duration_secs: popup.duration_seconds,
    };

    info!("弹窗：{} — {}", payload.origin, payload.body);

    state
        .pending_toasts
        .lock()
        .unwrap()
        .insert(label.clone(), payload);

    let Some(monitor) = app.primary_monitor().ok().flatten() else {
        warn!("拿不到主显示器，弹窗取消");
        return;
    };

    let scale = monitor.scale_factor();
    let _ = scale; // 尺寸用 DIP（窗口自己换算）；位置用物理像素（见 reposition_all）

    let builder = WebviewWindowBuilder::new(app, &label, WebviewUrl::App("toast.html".into()))
        .title("SmsPop")
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .always_on_top(true)
        .skip_taskbar(true)
        // ★ 不抢焦点：弹窗出现 / 被点击都不能动用户的前台窗口。
        .focusable(false)
        .focused(false)
        .resizable(false)
        .inner_size(popup.width, TOAST_HEIGHT)
        .visible(false);

    let window = match builder.build() {
        Ok(window) => window,
        Err(error) => {
            warn!("创建 toast 窗口失败：{error}");
            state.pending_toasts.lock().unwrap().remove(&label);
            return;
        }
    };

    {
        let mut open = state.toasts.lock().unwrap();
        open.push_back(label.clone());
    }

    // 窗口销毁（不管哪条路关的）→ 从堆里摘掉并重排
    let app_for_event = app.clone();
    let label_for_event = label.clone();
    window.on_window_event(move |event| {
        if matches!(event, WindowEvent::Destroyed) {
            on_toast_destroyed(&app_for_event, &label_for_event);
        }
    });

    reposition_all(app);
    let _ = window.show();

    // 兜底定时关闭（前端崩溃/事件丢失也不至于留着窗口）
    let app_for_timer = app.clone();
    let label_for_timer = label.clone();
    let duration = u64::from(popup.duration_seconds).max(2) + 2;
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_secs(duration)).await;
        close_toast(&app_for_timer, &label_for_timer);
    });
}

/// 关掉一个 toast（用户点掉 / 超时 / 被挤掉）。
pub fn close_toast(app: &AppHandle, label: &str) {
    if let Some(window) = app.get_webview_window(label) {
        let _ = window.close();
    } else {
        // 窗口已经没了，补一次清理
        on_toast_destroyed(app, label);
    }
}

fn on_toast_destroyed(app: &AppHandle, label: &str) {
    let state = app.state::<AppState>();

    let mut open = state.toasts.lock().unwrap();
    let removed = open
        .iter()
        .position(|item| item == label)
        .map(|index| open.remove(index));
    state.pending_toasts.lock().unwrap().remove(label);
    drop(open);

    if removed.is_some() {
        reposition_all(app);
    }
}

/// 按「最新的在最下面」重排所有 toast。
fn reposition_all(app: &AppHandle) {
    let state = app.state::<AppState>();
    let config = state.config();

    let Some(monitor) = app.primary_monitor().ok().flatten() else {
        return;
    };

    let work_area = monitor.work_area();
    let scale = monitor.scale_factor();
    let width_px = (config.notifications.popup.width * scale) as i32;
    let height_px = (TOAST_HEIGHT * scale) as i32;
    let margin_px = (MARGIN * scale) as i32;
    let gap_px = (GAP * scale) as i32;

    let open = state.toasts.lock().unwrap();

    for (slot, label) in open.iter().rev().enumerate() {
        let x = work_area.position.x + work_area.size.width as i32 - width_px - margin_px;
        let y = work_area.position.y + work_area.size.height as i32
            - margin_px
            - (slot as i32 + 1) * height_px
            - slot as i32 * gap_px;

        if let Some(window) = app.get_webview_window(label) {
            let _ = window.set_position(Position::Physical(PhysicalPosition::new(x, y)));
        }
    }
}
