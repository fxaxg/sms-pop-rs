//! 蓝牙链路线程：把 `LinkEvent` 翻译成应用动作（弹窗 / 复制 / 候选条 / 状态广播）。

use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

use log::{info, warn};
use smspop_ble::{AncsLink, LinkEvent, LinkState};
use smspop_core::model::PhoneNotification;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_clipboard_manager::ClipboardExt;

use crate::state::AppState;
use crate::types::LinkStatePayload;
use crate::{caret, popups, tray};

/// 启动 ANCS 监督循环（独立线程，断了会自己重连）。
pub fn spawn(app: AppHandle) {
    let stop = app.state::<AppState>().link_stop.clone();

    let result = std::thread::Builder::new()
        .name("ancs-link".to_string())
        .spawn(move || {
            let sink_app = app.clone();
            let sink = Arc::new(move |event: LinkEvent| handle_event(&sink_app, event));

            AncsLink::new(sink, stop).run();
        });

    if let Err(error) = result {
        warn!("蓝牙链路线程启动失败：{error}");
    }
}

fn handle_event(app: &AppHandle, event: LinkEvent) {
    match event {
        LinkEvent::StateChanged { state, detail } => on_link_state(app, state, detail),
        LinkEvent::Notification(notification) => {
            verify_link(app);
            dispatch_notification(app, *notification);
        }
    }
}

fn on_link_state(app: &AppHandle, state: LinkState, detail: Option<String>) {
    info!(
        "链路状态：{}（{}）",
        state.describe(),
        detail.as_deref().unwrap_or("-")
    );

    {
        let app_state = app.state::<AppState>();
        let mut guard = app_state
            .link_state
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        *guard = (state, detail.clone());

        // 每次离开已订阅状态都结束本轮验证；下次订阅必须重新收到真实数据。
        if state != LinkState::Subscribed {
            app_state.link_verified.store(false, Ordering::Relaxed);
        }

        if state == LinkState::Subscribed {
            *app_state.subscribed_at.lock().unwrap() = Some(std::time::Instant::now());
        }
    }

    let verified = state == LinkState::Subscribed
        && app
            .state::<AppState>()
            .link_verified
            .load(Ordering::Relaxed);
    tray::on_link_state(app, state, verified);

    let payload = LinkStatePayload {
        state: crate::types::state_key(state).to_string(),
        label: state.describe().to_string(),
        detail,
        ready: verified,
        awaiting_verification: state == LinkState::Subscribed && !verified,
    };

    if let Err(error) = app.emit(crate::types::EVENT_LINK_STATE, payload) {
        warn!("广播链路状态失败：{error}");
    }
}

/// 第一条真实 ANCS 数据到达后，把「已订阅」提升为「已验证」。
/// 本地测试通知不经过这里，因此不会伪造链路成功。
fn verify_link(app: &AppHandle) {
    let app_state = app.state::<AppState>();
    let (state, detail) = app_state
        .link_state
        .read()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clone();
    if state != LinkState::Subscribed || app_state.link_verified.swap(true, Ordering::Relaxed) {
        return;
    }

    info!("已收到真实 iPhone 通知，ANCS 链路验证通过");
    tray::on_link_state(app, state, true);
    let payload = LinkStatePayload {
        state: crate::types::state_key(state).to_string(),
        label: state.describe().to_string(),
        detail,
        ready: true,
        awaiting_verification: false,
    };
    if let Err(error) = app.emit(crate::types::EVENT_LINK_STATE, payload) {
        warn!("广播链路验证状态失败：{error}");
    }
}

/// 一条通知进来了。全走真实路径：去重 → 过滤 → 复制 / 弹窗 / 候选条。
pub fn dispatch_notification(app: &AppHandle, notification: PhoneNotification) {
    let state = app.state::<AppState>();

    // 订阅成功后的宽限期里按内容去重 —— iOS 会把通知中心存量用新 uid 重推一遍。
    let in_grace = state
        .subscribed_at
        .lock()
        .unwrap()
        .is_some_and(|t| t.elapsed() < Duration::from_secs(5));

    let duplicate = if in_grace {
        state
            .dedup
            .is_duplicate_key_at(&notification.content_key(), std::time::Instant::now())
    } else {
        state.dedup.is_duplicate(&notification)
    };

    if duplicate {
        info!("重复通知，忽略：{}", notification.summary());
        return;
    }

    let config = state.config();

    if !config.passes_filter(&notification) {
        info!("被过滤规则拦下：{}", notification.summary());
        return;
    }

    let offer_otp = config.should_offer_otp(&notification);

    // ① 验证码 → 剪贴板（纯被动，不碰任何程序）
    if offer_otp && config.otp.auto_copy {
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
    if offer_otp && config.otp.caret.enabled {
        if let Some(code) = notification.code.clone() {
            caret::watch(app, code);
        }
    }
}
