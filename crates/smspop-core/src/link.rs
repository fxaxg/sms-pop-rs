use crate::model::PhoneNotification;

/// 链路状态。托盘 tooltip 和状态对话框显示的就是它。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LinkState {
    /// 还没开始。
    #[default]
    Stopped,
    /// 本机蓝牙不支持 BLE 外设角色 —— 这条路走不通。
    AdapterUnsupported,
    /// 广播已经起来，等 iPhone 连过来。
    Advertising,
    /// 找到设备了，等链路建立。
    WaitingForConnection,
    /// 链路通了，正在订阅 ANCS。
    Connected,
    /// 已就绪，正在收通知。
    Subscribed,
    /// 断开后正在重连。
    Reconnecting,
    /// 出错了，退避后重试。
    Faulted,
}

impl LinkState {
    /// 人话描述（给托盘 tooltip 和状态对话框用）。
    pub fn describe(self) -> &'static str {
        match self {
            Self::Stopped => "已停止",
            Self::AdapterUnsupported => "本机蓝牙不支持",
            Self::Advertising => "等待 iPhone 连接",
            Self::WaitingForConnection => "等待链路建立",
            Self::Connected => "已连接，正在订阅",
            Self::Subscribed => "已就绪",
            Self::Reconnecting => "重连中",
            Self::Faulted => "出错",
        }
    }

    /// 已经能正常收通知了吗。
    pub fn is_ready(self) -> bool {
        self == Self::Subscribed
    }
}

/// 链路往外报的事件。
#[derive(Debug, Clone)]
pub enum LinkEvent {
    /// 状态变了。`detail` 是给人看的补充说明（可能为空）。
    StateChanged {
        state: LinkState,
        detail: Option<String>,
    },
    /// 收到一条通知（已经回读完正文）。
    Notification(Box<PhoneNotification>),
    /// 本轮枚举到的全部候选设备。
    DevicesDiscovered(Vec<DeviceInfo>),
    /// 当前实际选择的设备；断开时为 None。
    ActiveDevice(Option<DeviceInfo>),
    /// 可选的标准 BLE 电量百分比。读取不到时不会发送事件。
    BatteryLevel { device_id: String, level: u8 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceInfo {
    pub id: String,
    pub name: String,
    pub connected: bool,
}
