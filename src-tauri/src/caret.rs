//! 光标候选条。
//!
//! 收到验证码后开一个 **N 秒监听窗**（`caret.watch_seconds`）：期间每 350ms
//! 探测一次焦点，首次发现光标落进可写输入框就把「填入 xxxxxx」弹到光标旁。
//! —— 用户经常是收到通知才去点输入框，只认到达瞬间的焦点会漏掉大多数场景。
//!
//! **只有用户点它才会写入**（`smspop-uia` 的纪律）。
//!
//! 窗口只有一个（label = `caret`），反复 hide/show。前端量好自身尺寸后
//! 调 `caret_layout`，Rust 用 `placement::compute` 定最终位置再显示。

use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use log::{info, warn};
use smspop_core::placement::{self, PixelRect, RectD};
use smspop_uia::{FocusTarget, ProbeOutcome};
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

/// 监听窗内的探测间隔。
const WATCH_INTERVAL: Duration = Duration::from_millis(350);

/// 写入的超时。
const INSERT_TIMEOUT: Duration = Duration::from_millis(1500);

const WINDOW_LABEL: &str = "caret";
const EDGE_PADDING: f64 = 8.0;

/// 收到验证码：开一个监听窗，等焦点落进输入框再弹候选条。
///
/// 立即返回（监听跑在独立线程上）。新的验证码会取代旧的监听。
pub fn watch(app: &AppHandle, code: String, source_hint: Option<String>) {
    let state = app.state::<AppState>();
    let config = state.config();

    if state.uia.read().unwrap().is_none() {
        return; // UIA 不可用，候选功能整体缺席
    }

    let window_secs = u64::from(config.otp.caret.watch_seconds).clamp(5, 600);
    let duration_secs = config.otp.caret.duration_seconds;
    let gap = config.otp.caret.gap;

    // 涨代数：取消上一个监听、作废当前候选条
    let generation = state.caret_generation.fetch_add(1, Ordering::SeqCst) + 1;
    hide_window(app);

    let app = app.clone();
    let _ = std::thread::Builder::new()
        .name("caret-watch".to_string())
        .spawn(move || {
            watch_loop(
                &app,
                generation,
                code,
                source_hint,
                window_secs,
                duration_secs,
                gap,
            );
        });
}

/// 监听循环：每 350ms 探测一次，首次命中可写焦点就弹条。
///
/// 一条验证码只主动弹一次 —— 没点到就用剪贴板（通知到达时已自动复制）。
fn watch_loop(
    app: &AppHandle,
    generation: u64,
    code: String,
    source_hint: Option<String>,
    window_secs: u64,
    duration_secs: u32,
    gap: f64,
) {
    let deadline = Instant::now() + Duration::from_secs(window_secs);

    loop {
        if Instant::now() >= deadline {
            return;
        }

        // 被新验证码取代 / 用户手动收起 → 退出
        if current_generation(app) != generation {
            return;
        }

        let outcome = {
            let state = app.state::<AppState>();
            let guard = state.uia.read().unwrap();
            guard.as_ref().map(|uia| uia.probe(PROBE_TIMEOUT))
        };

        match outcome {
            Some(ProbeOutcome::Found(target)) if target.editable && target.caret.is_some() => {
                show_bar(
                    app,
                    generation,
                    code,
                    source_hint,
                    target,
                    duration_secs,
                    gap,
                );
                return;
            }
            // UIA 挂了（worker 退出）—— 再轮询也没意义
            Some(ProbeOutcome::Failed(error)) => {
                warn!("焦点探测失败：{error}");
                return;
            }
            _ => {}
        }

        std::thread::sleep(WATCH_INTERVAL);
    }
}

/// 把候选条递出去（此时已经确认焦点可写、光标位置已知）。
fn show_bar(
    app: &AppHandle,
    generation: u64,
    code: String,
    source_hint: Option<String>,
    target: FocusTarget,
    duration_secs: u32,
    gap: f64,
) {
    let state = app.state::<AppState>();

    // 显示前再确认一次没被取代
    if current_generation(app) != generation {
        return;
    }

    let caret_rect = target.caret.expect("调用前已确认 caret 存在");
    let dpi = target.dpi.max(96);
    let caret_dips = caret_rect.to_dips(dpi);
    let work_area = work_area_dips(caret_rect, dpi);

    {
        let mut offer = state.caret_offer.lock().unwrap();
        *offer = Some(CaretOffer {
            generation,
            code,
            source_hint,
            caret: caret_dips,
            work_area,
            scale: dpi as f64 / 96.0,
            gap,
            duration_secs,
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

    info!("候选条已递出（dpi={dpi}）");

    // 超时自动消失
    let app_for_timer = app.clone();
    let duration = u64::from(duration_secs).max(2);
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

    // placement 在 DIP 空间里算，换回物理像素喂给窗口。
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

/// 收起候选条（用户点完 / 超时 / 新验证码取代）。同时取消在途的监听。
pub fn hide(app: &AppHandle) {
    // 涨代数，让在途的定时器、布局请求和监听线程全部失效
    app.state::<AppState>()
        .caret_generation
        .fetch_add(1, Ordering::SeqCst);

    hide_window(app);
}

/// 只收窗口，不动代数（watch 内部换代时自己先涨过代数了）。
fn hide_window(app: &AppHandle) {
    *app.state::<AppState>().caret_offer.lock().unwrap() = None;

    if let Some(window) = app.get_webview_window(WINDOW_LABEL) {
        let _ = window.hide();
    }
}

pub fn hide_if_generation(app: &AppHandle, generation: u64) {
    if current_generation(app) != generation {
        return; // 期间来了新提议，别动
    }

    hide(app);
}

fn current_generation(app: &AppHandle) -> u64 {
    app.state::<AppState>()
        .caret_generation
        .load(Ordering::SeqCst)
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
