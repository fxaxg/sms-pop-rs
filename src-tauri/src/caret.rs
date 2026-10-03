//! 光标候选条。
//!
//! 收到验证码时探测一下焦点：光标落在可写输入框里，就在光标旁冒出一个
//! 「填入 xxxxxx」的小条。**只有用户点它才会写入**（`smspop-uia` 的纪律）。
//!
//! 窗口只有一个（label = `caret`），反复 hide/show。前端量好自身尺寸后
//! 调 `caret_layout`，Rust 用 `placement::compute` 定最终位置再显示。

use std::sync::atomic::Ordering;
use std::time::Duration;

use log::{info, warn};
use smspop_core::placement::{self, PixelRect, RectD};
use smspop_uia::ProbeOutcome;
use tauri::{
    AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, Position, Size, WebviewUrl,
    WebviewWindow, WebviewWindowBuilder,
};
use windows::Win32::Foundation::POINT;
use windows::Win32::Graphics::Gdi::{
    GetMonitorInfoW, MonitorFromPoint, MONITORINFO, MONITOR_DEFAULTTONEAREST,
};

use crate::state::AppState;
use crate::types::{CaretLayout, CaretOffer};

/// 探测焦点框的超时。卡住只是「这次不给候选」，绝不能拖住通知链路。
const PROBE_TIMEOUT: Duration = Duration::from_millis(900);

/// 写入的超时。
const INSERT_TIMEOUT: Duration = Duration::from_millis(1500);

const WINDOW_LABEL: &str = "caret";
const EDGE_PADDING: f64 = 8.0;

/// 收到验证码：探测焦点，合适就把候选条递出去。
pub fn offer(app: &AppHandle, code: String) {
    let state = app.state::<AppState>();
    let config = state.config();

    let uia_guard = state
        .uia
        .read()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let Some(uia) = uia_guard.as_ref() else {
        return;
    };

    let target = match uia.probe(PROBE_TIMEOUT) {
        ProbeOutcome::Found(target) => target,
        ProbeOutcome::TimedOut => {
            info!("焦点探测超时，这次不给候选");
            return;
        }
        ProbeOutcome::Failed(error) => {
            warn!("焦点探测失败：{error}");
            return;
        }
    };

    if !target.editable {
        info!("焦点不可写，不给候选：{}", target.describe());
        return;
    }

    let Some(caret_rect) = target.caret else {
        info!("拿不到光标位置，不给候选：{}", target.describe());
        return;
    };

    let dpi = target.dpi.max(96);
    let caret_dips = caret_rect.to_dips(dpi);
    let work_area = work_area_dips(caret_rect, dpi);
    let scale = dpi as f64 / 96.0;

    let generation = state.caret_generation.fetch_add(1, Ordering::SeqCst) + 1;

    {
        let mut offer = state.caret_offer.lock().unwrap();
        *offer = Some(CaretOffer {
            generation,
            code,
            caret: caret_dips,
            work_area,
            scale,
            gap: config.otp.caret.gap,
            duration_secs: config.otp.caret.duration_seconds,
        });
    }

    if let Err(error) = ensure_window(app) {
        warn!("创建候选条窗口失败：{error}");
        return;
    }

    // 前端可能刚加载还没挂上监听 —— 没关系，它 mount 时会主动来取。
    if let Some(window) = app.get_webview_window(WINDOW_LABEL) {
        let _ = window.emit(crate::types::EVENT_CARET_OFFER, generation);
    }

    // 超时自动消失
    let app_for_timer = app.clone();
    let duration = u64::from(config.otp.caret.duration_seconds).max(2);
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_secs(duration)).await;
        hide_if_generation(&app_for_timer, generation);
    });
}

/// 前端量好了自己的尺寸 → 定位置、定尺寸、显示。
pub fn layout(app: &AppHandle, request: CaretLayout) {
    let state = app.state::<AppState>();

    let offer = {
        let guard = state.caret_offer.lock().unwrap();
        match guard.as_ref() {
            Some(offer) if offer.generation == request.generation => offer.clone(),
            _ => return, // 迟到的布局请求，忽略
        }
    };

    let Some(window) = app.get_webview_window(WINDOW_LABEL) else {
        return;
    };

    let (left, top) = placement::compute(
        offer.caret,
        request.width,
        request.height,
        offer.work_area,
        offer.gap,
        EDGE_PADDING,
    );

    let scale = offer.scale;
    let x = (left * scale) as i32;
    let y = (top * scale) as i32;
    let width = (request.width * scale).ceil() as u32;
    let height = (request.height * scale).ceil() as u32;

    // 先挪过去再改尺寸，让窗口按目标显示器的缩放解释尺寸。
    let _ = window.set_position(Position::Physical(PhysicalPosition::new(x, y)));
    let _ = window.set_size(Size::Physical(PhysicalSize::new(width, height)));
    let _ = window.show();
}

/// 用户点了「填入」。这是全项目唯一会写别的程序的入口。
pub fn insert(app: &AppHandle, generation: u64) -> Option<(bool, String)> {
    let state = app.state::<AppState>();

    let code = {
        let guard = state.caret_offer.lock().unwrap();
        match guard.as_ref() {
            Some(offer) if offer.generation == generation => offer.code.clone(),
            _ => return None,
        }
    };

    let outcome = {
        let guard = state
            .uia
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        match guard.as_ref() {
            Some(uia) => uia.insert(&code, INSERT_TIMEOUT),
            None => return Some((false, "UIA 不可用，请手动粘贴".to_string())),
        }
    };

    if outcome.success {
        info!("已把验证码写入焦点输入框");
    } else {
        warn!("写入失败：{}", outcome.message);
    }

    Some((outcome.success, outcome.message))
}

/// 收起候选条（用户点完 / 超时 / 点了别处）。
pub fn hide(app: &AppHandle) {
    let state = app.state::<AppState>();

    // 涨代数，让在途的定时器和布局请求失效
    state.caret_generation.fetch_add(1, Ordering::SeqCst);
    *state.caret_offer.lock().unwrap() = None;

    if let Some(window) = app.get_webview_window(WINDOW_LABEL) {
        let _ = window.hide();
    }
}

fn hide_if_generation(app: &AppHandle, generation: u64) {
    let state = app.state::<AppState>();

    let current = state.caret_generation.load(Ordering::SeqCst);
    if current != generation {
        return; // 期间来了新提议，别动
    }

    hide(app);
}

/// 懒创建候选条窗口。
fn ensure_window(app: &AppHandle) -> tauri::Result<WebviewWindow> {
    if let Some(window) = app.get_webview_window(WINDOW_LABEL) {
        return Ok(window);
    }

    WebviewWindowBuilder::new(app, WINDOW_LABEL, WebviewUrl::App("caret.html".into()))
        .title("SmsPop")
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .focusable(false)
        .focused(false)
        .resizable(false)
        .inner_size(10.0, 10.0)
        .visible(false)
        .build()
}

/// 光标所在显示器的工作区（DIP）。
fn work_area_dips(caret: PixelRect, dpi: u32) -> RectD {
    let point = POINT {
        x: caret.left,
        y: caret.top,
    };

    // 兜底：全屏大矩形，保证 clamp 有意义
    let fallback = RectD::new(0.0, 0.0, 4096.0, 2160.0);

    unsafe {
        let monitor = MonitorFromPoint(point, MONITOR_DEFAULTTONEAREST);
        let mut info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };

        if !GetMonitorInfoW(monitor, &mut info).as_bool() {
            return fallback;
        }

        let rect = info.rcWork;
        PixelRect::new(rect.left, rect.top, rect.right, rect.bottom).to_dips(dpi)
    }
}
