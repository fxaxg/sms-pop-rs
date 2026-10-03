//! 焦点探测与写入。
//!
//! 这是"预填"功能的执行层。**它不是自动输入** —— 理解这一点再看代码。
//!
//! ```text
//! iPhone 收到验证码
//!       ↓
//! ① 右下角弹通知                     ← smspop-ui 里，纯展示
//! ② 自动复制到剪贴板                  ← 纯被动，不碰任何程序
//!       ↓
//! ③ 看一眼："现在焦点落在可写的输入框里吗？"   ← 这个 crate 的 probe
//!       ├─ 不是 → 到此为止
//!       └─ 是   → 光标旁边冒出一个小条：[填入] 868740
//!                    ↓
//!              ④ 你点它 → 才真正写进输入框      ← 这个 crate 的 insert
//!                 你不点 → 自己消失，什么都不会变
//! ```
//!
//! ## 四条纪律
//!
//! 1. **UIA 调用可能因为目标程序忙而卡住好几秒**，所以全部在独立线程上带超时 ——
//!    绝不拖住 UI（托盘和弹窗卡住比"这次没给候选"严重得多）。
//! 2. **绝不记录输入框内容**，日志里只有长度。那个框里可能是密码。
//! 3. **拿不到光标就不给候选**，不猜位置 —— 宁可这次不给，也不要把窗口弹到莫名其妙的地方。
//! 4. 整个 crate 里唯一会改动别人内容的地方是 [`UiaWorker::insert`]，
//!    而它只在**用户点击候选条**时被调用。这里没有定时器、没有后台任务会走到那里。

mod insert;
mod interop;
mod probe;
mod typing;
mod worker;

pub use worker::UiaWorker;

pub use smspop_core::config::InsertMode;

use smspop_core::placement::PixelRect;

/// 探测到的焦点输入框。
///
/// 只带"数据"，不带 COM 接口 —— 接口留在工作线程里（见 crate 文档）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FocusTarget {
    /// 焦点是不是落在一个**可写**的文本框里。
    pub editable: bool,

    /// 不可写的原因（`editable` 为真时是 `None`）。会进日志。
    pub skip_reason: Option<String>,

    /// 输入框当前内容**有多长**。
    ///
    /// ★ 只有长度，没有内容。日志里也只允许出现长度。
    pub value_len: usize,

    pub is_read_only: bool,

    /// 光标位置（物理像素）。拿不到就是 `None`，此时不给候选。
    pub caret: Option<PixelRect>,

    /// 光标是从哪拿到的：`"uia"` 或 `"win32"`。
    pub caret_source: Option<&'static str>,

    /// 焦点所在显示器的 DPI。
    pub dpi: u32,

    /// 焦点窗口的类名。**仅用于日志排障**（比如记事本是 `RichEditD2DPT`）。
    pub class_name: Option<String>,
}

impl FocusTarget {
    /// 一行式的排障描述。
    ///
    /// ★ 刻意**不含输入框内容** —— 只有长度。
    pub fn describe(&self) -> String {
        format!(
            "editable={} readonly={} value_len={} caret={} dpi={} class={}",
            self.editable,
            self.is_read_only,
            self.value_len,
            self.caret_source.unwrap_or("-"),
            self.dpi,
            self.class_name.as_deref().unwrap_or("-"),
        )
    }
}

/// 一次探测的结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProbeOutcome {
    /// 探测到了（不一定可写，看 [`FocusTarget::editable`]）。
    Found(FocusTarget),

    /// 超过超时时间还没返回 —— 目标程序忙，这次跳过。
    TimedOut,

    /// 探测本身出错了。
    Failed(String),
}

/// 一次写入尝试的结果，用来给候选条显示反馈。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InsertOutcome {
    pub success: bool,
    pub message: String,
}

impl InsertOutcome {
    pub(crate) fn yes(message: impl Into<String>) -> Self {
        Self {
            success: true,
            message: message.into(),
        }
    }

    pub(crate) fn no(message: impl Into<String>) -> Self {
        Self {
            success: false,
            message: message.into(),
        }
    }
}
