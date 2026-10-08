//! 蓝牙链路线程：把 `LinkEvent` 翻译成应用动作（弹窗 / 复制 / 候选条 / 状态广播）。

use std::sync::atomic::Ordering;
use std::sync::Arc;

use log::{info, warn};
use smspop_ble::{AncsLink, DeviceInfo, LinkEvent, LinkState};
use tauri::{AppHandle, Emitter, Manager};

use crate::notification_dispatch::dispatch_notification;
use crate::state::AppState;
use crate::tray;
use crate::types::LinkStatePayload;

/// 启动 ANCS 监督循环（独立线程，断了会自己重连）。
pub fn spawn(app: AppHandle) {
    let stop = app.state::<AppState>().link_stop.clone();

    let result = std::thread::Builder::new()
        .name("ancs-link".to_string())
        .spawn(move || {
            let sink_app = app.clone();
            let selector_app = app.clone();
            let sink = Arc::new(move |event: LinkEvent| handle_event(&sink_app, event));

            let selector = Arc::new(move |devices: &[DeviceInfo]| {
                selector_app
                    .state::<AppState>()
                    .devices
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .discover(devices)
            });
            AncsLink::new(sink, stop)
                .with_device_selector(selector)
                .run();
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
        LinkEvent::DevicesDiscovered(devices) => {
            // 非空列表通常已经由 selector 更新过；空列表用于清掉上一轮在线快照。
            app.state::<AppState>()
                .devices
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .discover(&devices);
            emit_devices(app);
        }
        LinkEvent::ActiveDevice(device) => {
            app.state::<AppState>()
                .devices
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .set_active(device.as_ref());
            emit_devices(app);
        }
        LinkEvent::BatteryLevel { device_id, level } => {
            app.state::<AppState>()
                .devices
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .set_battery_level(&device_id, level);
            emit_devices(app);
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
    {
        let mut devices = app_state
            .devices
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(id) = devices.active_id() {
            devices.mark_verified(&id);
        }
    }
    emit_devices(app);
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

pub fn emit_devices(app: &AppHandle) {
    let devices = app
        .state::<AppState>()
        .devices
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .payloads();
    if let Err(error) = app.emit(crate::types::EVENT_DEVICES_CHANGED, devices) {
        warn!("广播设备列表失败：{error}");
    }
}
