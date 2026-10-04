//! SmsPop 的 Windows 蓝牙层。
//!
//! 只做一件事：**把 iPhone 的通知拿回来**。
//!
//! ANCS 的角色是交叉的，这一点必须先记住，否则后面看代码会晕：
//!
//! * iPhone = 链路层 Central + GATT **Server**
//! * 本机   = 链路层 Peripheral + GATT **Client**
//!
//! 所以我们得**既广播又当 GATT 服务端**（不然 iPhone 连不上、也就没法配对），
//! 同时还要当 GATT 客户端去订阅 iPhone 上的 ANCS 服务。
//!
//! 模块划分：
//!
//! * [`gatt_server`] —— 本机的可连接 GATT 服务端（含一个"需要加密"的特征，用来逼出配对）
//! * [`advertiser`] —— 广播 ANCS 服务请求（iPhone 靠它认出"对面是个 ANCS 配件"）
//! * [`discovery`] —— 找设备、等链路、取 ANCS 服务
//! * [`session`] —— 订阅、Control Point 串行化、Data Source 分片重组
//! * [`link`] —— 监督循环：广播 → 连接 → 订阅 → 守着 → 断了重来

pub mod advertiser;
pub mod discovery;
pub mod gatt_server;
pub mod link;
pub mod session;
pub mod util;

pub use link::{AncsLink, DeviceInfo, DeviceSelector, LinkEvent, LinkOptions, LinkState};
pub use session::NotificationSink;

/// 蓝牙层统一错误。
///
/// 分两类是为了让上层能给出**有用的**提示：
/// `AdapterUnsupported` 是要弹给用户看的（"你这台电脑蓝牙不支持"），
/// 其余是排障用的。
#[derive(Debug)]
pub enum BleError {
    /// 本机蓝牙不支持 BLE 外设角色 —— 这条路走不通，得让用户知道。
    AdapterUnsupported(String),
    /// WinRT 调用失败。
    WinRt(windows::core::Error),
    /// 我们自己判定的失败。
    Message(String),
}

impl std::fmt::Display for BleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AdapterUnsupported(detail) => write!(f, "{detail}"),
            Self::WinRt(error) => write!(f, "{error}"),
            Self::Message(message) => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for BleError {}

impl From<windows::core::Error> for BleError {
    fn from(error: windows::core::Error) -> Self {
        Self::WinRt(error)
    }
}

impl From<String> for BleError {
    fn from(message: String) -> Self {
        Self::Message(message)
    }
}

impl From<&str> for BleError {
    fn from(message: &str) -> Self {
        Self::Message(message.to_string())
    }
}

pub type Result<T> = std::result::Result<T, BleError>;
