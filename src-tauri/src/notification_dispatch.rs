#[cfg(windows)]
use crate::caret;
use crate::{popups, state::AppState};
use log::{info, warn};
use smspop_core::model::PhoneNotification;
use std::time::Duration;
use tauri::{AppHandle, Manager};
use tauri_plugin_clipboard_manager::ClipboardExt;

/// 一条通知进来了。全走真实路径：去重 → 过滤 → 复制 / 弹窗 / 候选条。
pub fn dispatch_notification(app: &AppHandle, notification: PhoneNotification) {
    dispatch_with_policy(app, notification, true, true);
}

/// 网络入口已自行按消息标识去重，不受 BLE 订阅宽限期影响。
pub fn dispatch_network(app: &AppHandle, notification: PhoneNotification, allow_copy: bool) {
    dispatch_with_policy(app, notification, false, allow_copy);
}

fn dispatch_with_policy(
    app: &AppHandle,
    notification: PhoneNotification,
    ble: bool,
    allow_copy: bool,
) {
    let state = app.state::<AppState>();

    // 订阅成功后的宽限期里按内容去重 —— iOS 会把通知中心存量用新 uid 重推一遍。
    let in_grace = state
        .subscribed_at
        .lock()
        .unwrap()
        .is_some_and(|t| t.elapsed() < Duration::from_secs(5));

    let duplicate = if !ble {
        false
    } else if in_grace {
        state
            .dedup
            .is_duplicate_key_at(&notification.content_key(), std::time::Instant::now())
    } else {
        state.dedup.is_duplicate(&notification)
    };

    if duplicate {
        info!("重复通知，忽略");
        return;
    }

    let config = state.config();

    if !config.passes_filter(&notification) {
        info!("通知被过滤规则拦下");
        return;
    }

    let offer_otp = config.should_offer_otp(&notification);
    if offer_otp {
        if let Some(code) = notification.code.clone() {
            state
                .otp_candidate
                .lock()
                .unwrap()
                .offer(code, std::time::Instant::now());
        }
    }

    // ① 验证码 → 剪贴板（纯被动，不碰任何程序）
    if offer_otp && config.otp.auto_copy && allow_copy {
        if let Some(code) = notification.code.as_deref() {
            if let Err(error) = app.clipboard().write_text(code.to_string()) {
                warn!("写剪贴板失败：{error}");
            }
        }
    }

    // ② 右下角弹窗
    if config.should_popup(&notification) {
        popups::show(app, &notification);
    }

    // ③ 光标候选条（点了才填入）。
    //    开 N 秒监听窗：等焦点落进输入框再弹，而不是只认到达瞬间。
    #[cfg(windows)]
    if offer_otp && config.otp.caret.enabled {
        if let Some(code) = notification.code.clone() {
            caret::watch(app, code, notification.otp_source_hint());
        }
    }
}
