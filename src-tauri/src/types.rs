//! 跨模块共享的负载类型与事件名。

use serde::{Deserialize, Serialize};
use smspop_core::placement::RectD;

/// 链路状态变化事件（Rust → 所有前端窗口）。
#[cfg(windows)]
pub const EVENT_LINK_STATE: &str = "link-state";
#[cfg(windows)]
pub const EVENT_DEVICES_CHANGED: &str = "devices-changed";

/// 候选条有新验证码（Rust → caret 窗口）。
#[cfg(windows)]
pub const EVENT_CARET_OFFER: &str = "caret-offer";

/// 链路状态 → 机器可读标识（前端按它判断，别看人话文案）。
pub fn state_key(state: smspop_core::link::LinkState) -> &'static str {
    use smspop_core::link::LinkState;

    match state {
        LinkState::Stopped => "stopped",
        LinkState::AdapterUnsupported => "adapter_unsupported",
        LinkState::Advertising => "advertising",
        LinkState::WaitingForConnection => "waiting",
        LinkState::Connected => "connected",
        LinkState::Subscribed => "subscribed",
        LinkState::Reconnecting => "reconnecting",
        LinkState::Faulted => "faulted",
    }
}

/// 链路状态负载。
#[derive(Debug, Clone, Serialize)]
pub struct LinkStatePayload {
    /// 机器可读的状态标识：`advertising` / `subscribed` / …
    pub state: String,
    /// 人话描述（「等待 iPhone 连接」）。
    pub label: String,
    /// 补充说明（错误原因等），可能为空。
    pub detail: Option<String>,
    /// 是否已就绪（订阅成功，并且真正收到过 iPhone 的 ANCS 数据）。
    pub ready: bool,
    /// ANCS 已订阅，但尚未用一条真实 iPhone 通知验证。
    pub awaiting_verification: bool,
}

/// 一条 toast 弹窗的展示负载。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToastPayload {
    pub app_id: Option<String>,
    pub open_app_name: Option<String>,
    /// 来源行（发件人 / App 名）。
    pub origin: String,
    /// 正文摘要。
    pub body: String,
    /// 识别出的验证码（没有则 `None`）。
    pub code: Option<String>,
    /// 停留秒数（前端倒计时条用；Rust 侧也有定时器兜底关闭）。
    pub duration_secs: u32,
}

/// 当前生效的候选条提议。
#[derive(Debug, Clone)]
#[cfg_attr(not(windows), allow(dead_code))]
pub struct CaretOffer {
    /// 单调递增的代数 —— 防止迟到的布局/隐藏请求误伤新提议。
    pub generation: u64,
    pub code: String,
    pub source_hint: Option<String>,
    /// 光标矩形（DIP）。
    pub caret: RectD,
    /// 光标所在显示器的工作区（DIP）。
    pub work_area: RectD,
    /// DIP → 物理像素的缩放（目标显示器 dpi / 96）。
    pub scale: f64,
    /// 与光标的间距（DIP）。
    pub gap: f64,
    /// 停留秒数。
    pub duration_secs: u32,
}

/// 前端请求的布局信息。
#[derive(Debug, Clone, Copy, Deserialize)]
#[cfg_attr(not(windows), allow(dead_code))]
pub struct CaretLayout {
    pub generation: u64,
    pub width: f64,
    pub height: f64,
}
