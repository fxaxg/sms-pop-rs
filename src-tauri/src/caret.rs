//! 有期限的输入辅助：默认手动候选条；显式开启后自动尝试一次。
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::{Duration, Instant};

use log::{info, warn};
use smspop_core::{
    caret::Tracker,
    placement::{self, PixelRect, RectD},
};
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

const PROBE_TIMEOUT: Duration = Duration::from_millis(900);
const WATCH_INTERVAL: Duration = Duration::from_millis(350);
const INSERT_TIMEOUT: Duration = Duration::from_millis(1500);
const WINDOW_LABEL: &str = "caret";
const EDGE_PADDING: f64 = 8.0;

struct Session {
    id: u64,
    active: Arc<AtomicBool>,
    deadline: Instant,
    claimed: bool,
}

#[derive(Default)]
pub struct CaretState {
    sequence: u64,
    offer_sequence: u64,
    session: Option<Session>,
    pub offer: Option<CaretOffer>,
}

impl CaretState {
    pub(crate) fn cancel(&mut self) {
        if let Some(session) = self.session.take() {
            session.active.store(false, Ordering::SeqCst);
        }
        self.offer = None;
    }
    fn watching(&self, id: u64) -> bool {
        self.session.as_ref().is_some_and(|s| {
            s.id == id
                && !s.claimed
                && s.active.load(Ordering::SeqCst)
                && Instant::now() < s.deadline
        })
    }
    fn claim(&mut self, id: u64) -> Option<Arc<AtomicBool>> {
        if !self.watching(id) {
            return None;
        }
        let session = self.session.as_mut()?;
        session.claimed = true;
        Some(session.active.clone())
    }
    fn cancel_session(&mut self, id: u64) -> bool {
        if self.session.as_ref().is_none_or(|session| session.id != id) {
            return false;
        }
        self.cancel();
        true
    }
    fn expire_offer(&mut self, generation: u64, now: Instant) -> bool {
        if self
            .offer
            .as_ref()
            .is_none_or(|offer| offer.generation != generation || now < offer.expires)
        {
            return false;
        }
        self.offer = None;
        true
    }
}

struct Watch {
    id: u64,
    code: String,
    source: Option<String>,
    started: Instant,
    lifetime: Duration,
    duration: Duration,
    gap: f64,
    automatic: bool,
}

pub fn watch(app: &AppHandle, code: String, source_hint: Option<String>) {
    let state = app.state::<AppState>();
    let config = state.config.read().unwrap();
    if state.uia.read().unwrap().is_none() {
        return;
    }
    if !config.otp.enabled || (!config.otp.auto_fill && !config.otp.caret.enabled) {
        return;
    }
    let lifetime = Duration::from_secs(u64::from(config.otp.caret.watch_seconds).clamp(5, 600));
    let started = Instant::now();
    let id = {
        let mut caret = state.caret.lock().unwrap();
        caret.cancel();
        caret.sequence += 1;
        let id = caret.sequence;
        caret.session = Some(Session {
            id,
            active: Arc::new(AtomicBool::new(true)),
            deadline: started + lifetime,
            claimed: false,
        });
        id
    };
    hide_native(app);
    let watch = Watch {
        id,
        code,
        source: source_hint,
        started,
        lifetime,
        duration: Duration::from_secs(u64::from(config.otp.caret.duration_seconds).clamp(2, 60)),
        gap: config.otp.caret.gap,
        automatic: config.otp.auto_fill,
    };
    drop(config);
    // A hard deadline also cancels slow probes/in-flight writes without extending the session.
    let timer_app = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep((started + lifetime).saturating_duration_since(Instant::now())).await;
        finish_session(&timer_app, id);
    });
    let worker_app = app.clone();
    if let Err(error) = std::thread::Builder::new()
        .name("caret-watch".into())
        .spawn(move || watch_loop(&worker_app, watch))
    {
        warn!("启动输入辅助失败：{error}");
        finish_session(app, id);
    }
}

fn watch_loop(app: &AppHandle, watch: Watch) {
    let mut tracker = Tracker::new(watch.lifetime, watch.duration);
    loop {
        if !app
            .state::<AppState>()
            .caret
            .lock()
            .unwrap()
            .watching(watch.id)
        {
            return;
        }
        let outcome = {
            let state = app.state::<AppState>();
            let uia = state.uia.read().unwrap();
            uia.as_ref()
                .map(|uia| uia.probe(&watch.code, PROBE_TIMEOUT))
        };
        // Probes may outlive their caller/session. Never act on a late result.
        if !app
            .state::<AppState>()
            .caret
            .lock()
            .unwrap()
            .watching(watch.id)
        {
            return;
        }
        let target = match outcome {
            Some(ProbeOutcome::Found(target)) if target.editable => Some(target),
            Some(ProbeOutcome::Failed(error)) => {
                warn!("焦点探测失败：{error}");
                finish_session(app, watch.id);
                return;
            }
            _ => None,
        };
        if watch.automatic {
            if let Some(target) = target.filter(|target| target.auto_fill_eligible) {
                let active = app
                    .state::<AppState>()
                    .caret
                    .lock()
                    .unwrap()
                    .claim(watch.id);
                if let Some(active) = active {
                    let state = app.state::<AppState>();
                    let uia = state.uia.read().unwrap();
                    if let Some(uia) = uia.as_ref() {
                        let result =
                            uia.insert(&watch.code, target.target_id, true, active, INSERT_TIMEOUT);
                        info!("自动填入尝试结束（success={}）", result.success);
                    }
                }
                finish_session(app, watch.id);
                return; // One attempt, even if validation, typing or a provider fails.
            }
        } else {
            let target = target.filter(|target| target.caret.is_some());
            let decision = tracker.observe(
                watch.started.elapsed(),
                target.as_ref().map(|t| t.target_id),
            );
            match decision {
                None => {
                    finish_session(app, watch.id);
                    return;
                }
                Some(decision) if decision.visible => {
                    if let Some(target) = target {
                        show_bar(app, &watch, target, watch.started + decision.until);
                    }
                }
                _ => hide_offer(app, watch.id),
            }
        }
        std::thread::sleep(WATCH_INTERVAL);
    }
}

fn show_bar(app: &AppHandle, watch: &Watch, target: FocusTarget, expires: Instant) {
    let Some(rect) = target.caret else {
        return;
    };
    if let Err(error) = ensure_window(app) {
        warn!("创建候选条窗口失败：{error}");
        finish_session(app, watch.id);
        return;
    }
    let dpi = target.dpi.max(96);
    let caret_rect = rect.to_dips(dpi);
    let work_area = work_area_dips(rect, dpi);
    let state = app.state::<AppState>();
    let mut caret = state.caret.lock().unwrap();
    if !caret.watching(watch.id) || Instant::now() >= expires {
        return;
    }
    if let Some(offer) = caret.offer.as_ref() {
        if offer.target_id == target.target_id
            && offer.caret == caret_rect
            && offer.work_area == work_area
            && offer.scale == dpi as f64 / 96.0
        {
            return;
        }
    }
    let new_target = caret
        .offer
        .as_ref()
        .is_none_or(|offer| offer.target_id != target.target_id);
    let generation = if new_target {
        caret.offer_sequence += 1;
        caret.offer_sequence
    } else {
        caret.offer.as_ref().unwrap().generation
    };
    caret.offer = Some(CaretOffer {
        session_id: watch.id,
        generation,
        target_id: target.target_id,
        expires,
        code: watch.code.clone(),
        source_hint: watch.source.clone(),
        caret: caret_rect,
        work_area,
        scale: dpi as f64 / 96.0,
        gap: watch.gap,
        duration_secs: watch.duration.as_secs() as u32,
    });
    drop(caret);
    if let Some(window) = app.get_webview_window(WINDOW_LABEL) {
        let _ = window.emit(crate::types::EVENT_CARET_OFFER, generation);
    }
    if new_target {
        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(expires.saturating_duration_since(Instant::now())).await;
            let state = app.state::<AppState>();
            let mut caret = state.caret.lock().unwrap();
            if caret.expire_offer(generation, Instant::now()) {
                drop(caret);
                hide_native(&app);
            }
        });
    }
}

/// Layout, dismissals and focus changes share a lock so late callbacks cannot show an old offer.
pub fn layout(app: &AppHandle, request: CaretLayout) {
    let app_handle = app.clone();
    let _ = app.run_on_main_thread(move || layout_on_main(&app_handle, request));
}

fn layout_on_main(app: &AppHandle, request: CaretLayout) {
    let state = app.state::<AppState>();
    let caret = state.caret.lock().unwrap();
    let Some(offer) = caret
        .offer
        .as_ref()
        .filter(|o| o.generation == request.generation && Instant::now() < o.expires)
    else {
        return;
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
    let _ = window.set_position(Position::Physical(PhysicalPosition::new(
        (left * scale) as i32,
        (top * scale) as i32,
    )));
    let _ = window.set_size(Size::Physical(PhysicalSize::new(
        (request.width * scale).ceil() as u32,
        (request.height * scale).ceil() as u32,
    )));
    let _ = window.show();
}

pub fn insert(app: &AppHandle, generation: u64) -> Option<(bool, String)> {
    let state = app.state::<AppState>();
    let (offer, active) = {
        let mut caret = state.caret.lock().unwrap();
        let offer = caret
            .offer
            .as_ref()
            .filter(|o| o.generation == generation && Instant::now() < o.expires)?
            .clone();
        let id = caret.session.as_ref()?.id;
        let active = caret.claim(id)?;
        (offer, active)
    };
    let result = {
        let uia = state.uia.read().unwrap();
        let uia = uia.as_ref()?;
        uia.insert(&offer.code, offer.target_id, false, active, INSERT_TIMEOUT)
    };
    info!("手动填入尝试结束（success={}）", result.success);
    Some((result.success, result.message))
}

pub fn config_applied(app: &AppHandle) {
    hide_native(app);
}

pub fn hide_if_generation(app: &AppHandle, generation: u64) {
    let state = app.state::<AppState>();
    let mut caret = state.caret.lock().unwrap();
    if caret
        .offer
        .as_ref()
        .is_some_and(|o| o.generation == generation)
    {
        caret.cancel();
        drop(caret);
        hide_native(app);
    }
}

fn finish_session(app: &AppHandle, id: u64) {
    let state = app.state::<AppState>();
    let mut caret = state.caret.lock().unwrap();
    if caret.cancel_session(id) {
        drop(caret);
        hide_native(app);
    }
}

fn hide_offer(app: &AppHandle, id: u64) {
    let state = app.state::<AppState>();
    let mut caret = state.caret.lock().unwrap();
    if caret.watching(id) && caret.offer.take().is_some() {
        drop(caret);
        hide_native(app);
    }
}

fn hide_native(app: &AppHandle) {
    let app_handle = app.clone();
    let _ = app.run_on_main_thread(move || {
        let state = app_handle.state::<AppState>();
        let caret = state.caret.lock().unwrap();
        // A newer offer may already exist by the time this UI task runs.
        if caret.offer.is_none() {
            if let Some(window) = app_handle.get_webview_window(WINDOW_LABEL) {
                let _ = window.hide();
            }
        }
    });
}

/// Explicit dismissal applies to this code's session, including a simultaneous focus handoff.
pub fn dismiss(app: &AppHandle, session: u64) {
    finish_session(app, session);
}

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

fn work_area_dips(caret: PixelRect, dpi: u32) -> RectD {
    let point = POINT {
        x: caret.left,
        y: caret.top,
    };
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn 旧会话关闭和旧停留定时器不会取消新目标() {
        let active = Arc::new(AtomicBool::new(true));
        let now = Instant::now();
        let mut state = CaretState {
            session: Some(Session {
                id: 2,
                active: active.clone(),
                deadline: now + Duration::from_secs(60),
                claimed: false,
            }),
            offer: Some(CaretOffer {
                session_id: 2,
                generation: 3,
                target_id: 9,
                expires: now + Duration::from_secs(8),
                code: "123456".into(),
                source_hint: None,
                caret: RectD::new(0.0, 0.0, 1.0, 1.0),
                work_area: RectD::new(0.0, 0.0, 1920.0, 1080.0),
                scale: 1.0,
                gap: 24.0,
                duration_secs: 8,
            }),
            ..Default::default()
        };
        assert!(!state.cancel_session(1));
        assert!(!state.expire_offer(2, now + Duration::from_secs(10)));
        assert!(!state.expire_offer(3, now));
        assert!(state.watching(2));
        assert!(state.expire_offer(3, now + Duration::from_secs(8)));
        assert!(state.offer.is_none());
        assert!(state.watching(2)); // presentation expiry does not cancel tracking
        assert!(state.cancel_session(2));
        assert!(!active.load(Ordering::SeqCst));
    }
    #[test]
    fn 每个会话最多认领一次且关闭会取消已排队的写入() {
        let active = Arc::new(AtomicBool::new(true));
        let mut state = CaretState {
            session: Some(Session {
                id: 1,
                active: active.clone(),
                deadline: Instant::now() + Duration::from_secs(60),
                claimed: false,
            }),
            ..Default::default()
        };
        assert!(state.claim(2).is_none());
        assert!(state.claim(1).is_some());
        assert!(state.claim(1).is_none());
        state.cancel();
        assert!(!active.load(Ordering::SeqCst));
        assert!(!state.watching(1));
    }

    #[test]
    fn 超时会话不能写入() {
        let mut state = CaretState {
            session: Some(Session {
                id: 1,
                active: Arc::new(AtomicBool::new(true)),
                deadline: Instant::now(),
                claimed: false,
            }),
            ..Default::default()
        };
        assert!(state.claim(1).is_none());
    }
}
