//! 右下角通知弹窗（toast）的窗口管理。
//!
//! 每条通知是一个独立的小窗口：无边框、透明、置顶、不抢焦点。
//! 堆叠在屏幕右下角，新的在最下面，超上限先关最旧的。
//!
//! **高度自适应**：窗口先按默认高度隐藏创建，前端渲染完内容后量出真实高度
//! 调 `toast_resize` —— Rust 记录高度、重排堆叠、再把窗口显示出来。
//! 内容两行三行都不会被裁。

use std::sync::atomic::Ordering;
use std::time::Duration;

use log::{info, warn};
use smspop_core::model::PhoneNotification;
use tauri::{
    AppHandle, Manager, PhysicalPosition, PhysicalSize, Position, Size, WebviewUrl,
    WebviewWindowBuilder, WindowEvent,
};

use crate::state::AppState;
use crate::types::ToastPayload;

/// 刚创建时的占位高度（DIP；马上会被前端报上来的真实高度替换）。
const DEFAULT_HEIGHT: f64 = 96.0;

/// 高度下限/上限（DIP）。上限防止超长通知占满屏幕。
const MIN_HEIGHT: f64 = 56.0;
const MAX_HEIGHT: f64 = 240.0;

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
        app_id: notification.app_identifier.clone(),
        open_app_name: smspop_core::app_rules::matching_rule(
            &config.notifications.app_rules,
            notification.app_identifier.as_deref(),
        )
        .map(|rule| rule.name.clone()),
        origin: notification.origin(),
        body: notification.summary(),
        code: notification.code.clone(),
        duration_secs: popup.duration_seconds,
    };

    info!("显示通知弹窗");

    state
        .pending_toasts
        .lock()
        .unwrap()
        .insert(label.clone(), payload);
    state
        .toast_heights
        .lock()
        .unwrap()
        .insert(label.clone(), DEFAULT_HEIGHT);

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
        .inner_size(popup.width, DEFAULT_HEIGHT)
        .visible(false);

    let window = match builder.build() {
        Ok(window) => window,
        Err(error) => {
            warn!("创建 toast 窗口失败：{error}");
            state.pending_toasts.lock().unwrap().remove(&label);
            state.toast_heights.lock().unwrap().remove(&label);
            return;
        }
    };
    info!("通知窗口已创建: {label}");

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

    // 先按占位高度摆好，但**不显示** —— 等前端报真实高度后再 show（见 resize_and_show）
    reposition_all(app);

    // 兜底定时关闭（前端崩溃/事件丢失也不至于留着窗口）
    let app_for_timer = app.clone();
    let label_for_timer = label.clone();
    let duration = u64::from(popup.duration_seconds).max(2) + 2;
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_secs(duration)).await;
        close_toast(&app_for_timer, &label_for_timer);
    });
}

/// 前端量出了真实高度 → 记高度、改尺寸、重排、显示。
pub fn resize_and_show(app: &AppHandle, label: &str, height_dip: f64) {
    info!("通知前端已就绪: {label}");
    let height = height_dip.clamp(MIN_HEIGHT, MAX_HEIGHT);
    let state = app.state::<AppState>();

    state
        .toast_heights
        .lock()
        .unwrap()
        .insert(label.to_string(), height);

    let Some(window) = app.get_webview_window(label) else {
        return;
    };

    let scale = window.scale_factor().unwrap_or(1.0);
    let width = state.config().notifications.popup.width;
    let _ = window.set_size(Size::Physical(PhysicalSize::new(
        (width * scale).ceil() as u32,
        (height * scale).ceil() as u32,
    )));

    reposition_all(app);
    match window.show() {
        Ok(()) => info!("通知窗口已显示: {label}"),
        Err(error) => warn!("显示通知窗口失败: {error}"),
    }
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
    state.toast_heights.lock().unwrap().remove(label);
    drop(open);

    if removed.is_some() {
        reposition_all(app);
    }
}

/// 按「最新的在最下面」重排所有 toast（高度各算各的）。
fn reposition_all(app: &AppHandle) {
    let state = app.state::<AppState>();
    let config = state.config();

    let Some(monitor) = app.primary_monitor().ok().flatten() else {
        return;
    };

    let work_area = monitor.work_area();
    let scale = monitor.scale_factor();
    let width_px = (config.notifications.popup.width * scale) as i32;
    let margin_px = (MARGIN * scale) as i32;
    let gap_px = (GAP * scale) as i32;

    let open = state.toasts.lock().unwrap();
    let heights = state.toast_heights.lock().unwrap();

    // 从底往上堆：最新的贴底，旧的依次往上
    let mut bottom = work_area.position.y + work_area.size.height as i32 - margin_px;

    for label in open.iter().rev() {
        let height_px = (heights.get(label).copied().unwrap_or(DEFAULT_HEIGHT) * scale) as i32;
        let x = work_area.position.x + work_area.size.width as i32 - width_px - margin_px;
        let y = bottom - height_px;

        if let Some(window) = app.get_webview_window(label) {
            let _ = window.set_position(Position::Physical(PhysicalPosition::new(x, y)));
        }

        bottom = y - gap_px;
    }
}
