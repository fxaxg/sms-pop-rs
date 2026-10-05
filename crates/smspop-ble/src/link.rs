//! 监督循环：广播 → 连接 → 订阅 → 守着 → 断了重来。
//!
//! 这一层是"让整个东西一直能用"的地方。BLE 链路一定会断
//! （iPhone 锁屏久了、走出去了、蓝牙重启了），
//! 断了不自己接回来，这个工具就没法当常驻程序用。
//!
//! 整条循环跑在**一条独立线程**上，通过 [`LinkEvent`] 往外报事件；
//! 上层（UI）拿到事件后自己决定怎么切线程。

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use log::{info, warn};
use smspop_core::model::PhoneNotification;
use windows::Devices::Bluetooth::BluetoothAdapter;
use windows::Win32::System::Com::{CoInitializeEx, COINIT_MULTITHREADED};

use crate::advertiser;
use crate::discovery::{self, sleep_unless_stopped, DiscoveredDevice};
use crate::gatt_server;
use crate::session::{self, NotificationSink};
use crate::util::block_on_timeout;
use crate::{BleError, Result};

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

impl From<&DiscoveredDevice> for DeviceInfo {
    fn from(device: &DiscoveredDevice) -> Self {
        Self {
            id: device.id.clone(),
            name: device.name.clone(),
            connected: device.connected,
        }
    }
}

pub type DeviceSelector = Arc<dyn Fn(&[DeviceInfo]) -> Vec<String> + Send + Sync>;

/// 各个环节的超时与退避。
#[derive(Debug, Clone)]
pub struct LinkOptions {
    /// 找设备的上限。找不到就报错重来。
    pub find_device_timeout: Duration,
    /// 等 iPhone 连上来的上限。
    pub connect_timeout: Duration,
    /// 正常断开后的重连间隔。
    pub reconnect_delay: Duration,
    /// 出错后的退避。
    pub error_backoff: Duration,
    /// 蓝牙不支持时的重试间隔（用户可能只是把蓝牙关了）。
    pub adapter_retry_delay: Duration,
}

impl Default for LinkOptions {
    fn default() -> Self {
        Self {
            find_device_timeout: Duration::from_secs(60),
            connect_timeout: Duration::from_secs(120),
            reconnect_delay: Duration::from_secs(2),
            error_backoff: Duration::from_secs(10),
            adapter_retry_delay: Duration::from_secs(30),
        }
    }
}

/// 一轮循环是怎么结束的。
enum RoundEnd {
    /// 被要求停止。
    Stopped,
    /// 链路断开了，应该马上重连。
    Disconnected,
}

/// ANCS 链路。
pub struct AncsLink {
    options: LinkOptions,
    sink: Arc<dyn Fn(LinkEvent) + Send + Sync>,
    stop: Arc<AtomicBool>,
    selector: DeviceSelector,
}

impl AncsLink {
    pub fn new(sink: Arc<dyn Fn(LinkEvent) + Send + Sync>, stop: Arc<AtomicBool>) -> Self {
        Self {
            options: LinkOptions::default(),
            sink,
            stop,
            selector: Arc::new(|devices| devices.iter().map(|device| device.id.clone()).collect()),
        }
    }

    pub fn with_options(mut self, options: LinkOptions) -> Self {
        self.options = options;
        self
    }

    pub fn with_device_selector(mut self, selector: DeviceSelector) -> Self {
        self.selector = selector;
        self
    }

    /// 跑监督循环。**阻塞**，直到 [`AncsLink::request_stop`] 被调用。
    ///
    /// 这个方法会跑在调用方的线程上（惯例是单独开一条 "ancs-link" 线程），
    /// 所以**它自己负责给这条线程初始化 COM** —— WinRT 的调用是本线程的事，
    /// 不能指望主线程替它初始化。
    pub fn run(&self) {
        // 没有 UI 的线程用 MTA 就够了（而且 MTA 下回调会走线程池，不需要消息泵）。
        // `ok()` 里包含 S_FALSE（已经初始化过），那也算成功。
        let com_ready = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }.ok();

        if let Err(error) = com_ready {
            warn!("初始化 COM 失败：{error} —— 蓝牙大概不会工作");
        }

        info!("ANCS 链路监督循环启动");

        let mut first_round = true;

        loop {
            if self.stopped() {
                self.emit(LinkState::Stopped, None);
                return;
            }

            if !first_round {
                self.emit(LinkState::Reconnecting, None);
            }
            first_round = false;

            match self.one_round() {
                Ok(RoundEnd::Stopped) => {
                    self.emit(LinkState::Stopped, None);
                    info!("ANCS 链路监督循环退出");
                    return;
                }
                Ok(RoundEnd::Disconnected) => {
                    info!("链路断开，{:?} 后重连", self.options.reconnect_delay);
                    sleep_unless_stopped(self.options.reconnect_delay, &self.stop);
                }
                Err(BleError::AdapterUnsupported(detail)) => {
                    warn!("{detail}");
                    self.emit(LinkState::AdapterUnsupported, Some(detail));
                    sleep_unless_stopped(self.options.adapter_retry_delay, &self.stop);
                }
                Err(error) => {
                    warn!("链路出错：{error}");
                    self.emit(LinkState::Faulted, Some(error.to_string()));
                    sleep_unless_stopped(self.options.error_backoff, &self.stop);
                }
            }
        }
    }

    pub fn request_stop(&self) {
        self.stop.store(true, Ordering::Relaxed);
    }

    fn stopped(&self) -> bool {
        self.stop.load(Ordering::Relaxed)
    }

    fn emit(&self, state: LinkState, detail: Option<String>) {
        (self.sink)(LinkEvent::StateChanged { state, detail });
    }

    /// 建服务端 + 广播 + 连 + 订阅 + 守到断开。
    fn one_round(&self) -> Result<RoundEnd> {
        check_adapter()?;

        // 广播和 GATT 服务端都必须活到这一轮结束
        self.emit(LinkState::Advertising, None);
        let provider = gatt_server::start()?;
        let publisher = advertiser::start()?;

        let outcome = self.run_session(&provider, &publisher);

        advertiser::stop(&publisher);
        gatt_server::stop(&provider);
        drop(publisher);

        outcome
    }

    fn run_session(
        &self,
        provider: &gatt_server::GattServerHost,
        publisher: &advertiser::AncsAdvertiser,
    ) -> Result<RoundEnd> {
        // GATT 广播会和 ANCS 广播抢广告位，可能被挤成 Aborted，
        // 而 Aborted 期间本机不可连接 —— 所以每次进来都确认一次。
        gatt_server::ensure_advertising(provider)?;
        info!(
            "首次配对广播检查：GATT={:?} ANCS={:?}",
            provider.provider().AdvertisementStatus().ok(),
            publisher.status()
        );

        self.emit(LinkState::WaitingForConnection, None);

        // 进入新一轮发现时先清空上一轮的在线快照，避免设备已经离开却仍显示在线。
        (self.sink)(LinkEvent::DevicesDiscovered(Vec::new()));
        let mut devices = discovery::find_devices(self.options.find_device_timeout, &self.stop)?;
        let infos: Vec<DeviceInfo> = devices.iter().map(DeviceInfo::from).collect();
        let order = (self.selector)(&infos);
        (self.sink)(LinkEvent::DevicesDiscovered(infos.clone()));
        devices.sort_by_key(|device| {
            order
                .iter()
                .position(|id| id.eq_ignore_ascii_case(&device.id))
                .unwrap_or(usize::MAX)
        });
        devices.retain(|device| order.iter().any(|id| id.eq_ignore_ascii_case(&device.id)));

        if devices.is_empty() {
            return Err("发现了 iPhone，但所有设备都已在设备管理器中暂停".into());
        }

        let mut failures = Vec::new();
        for candidate in devices {
            match self.run_device_session(candidate) {
                Ok(outcome) => return Ok(outcome),
                Err(error) => {
                    (self.sink)(LinkEvent::ActiveDevice(None));
                    warn!("设备连接失败，尝试下一台：{error}");
                    failures.push(error.to_string());
                }
            }
        }

        Err(format!("所有可用 iPhone 都连接失败：{}", failures.join("；")).into())
    }

    fn run_device_session(&self, candidate: DiscoveredDevice) -> Result<RoundEnd> {
        let device = candidate.device;
        let device_id = candidate.id;
        let device_name = candidate.name;
        info!("选定设备：{device_name}（{device_id}）");
        (self.sink)(LinkEvent::ActiveDevice(Some(DeviceInfo {
            id: device_id.clone(),
            name: device_name.clone(),
            connected: false,
        })));

        // ★ 不再把 `ConnectionStatus` 当成硬门槛。
        //
        //   WinRT 的 `BluetoothLEDevice.ConnectionStatus` 反映的是"Windows 作为
        //   中心、对这台设备有没有活动连接"。冷启动时这个 GATT 客户端会话还没建立，
        //   它会一直报 Disconnected —— 于是死等在这里永远等不到（真机踩过：
        //   系统 UI 显示"已连接"，这里却一直超时）。
        //
        //   正确做法是直接去请求 ANCS 服务：那一步会主动建立 GATT 客户端连接，
        //   连接一建起来 `ConnectionStatus` 也就正常了。
        //
        //   所以这里只等一小会儿给链路沉降，然后无论如何都往下走。
        let settle_wait = self.options.connect_timeout.min(Duration::from_secs(5));

        match discovery::wait_for_connection(&device, settle_wait, &self.stop) {
            Ok(()) => info!("链路已连上"),
            Err(error) => {
                info!("链路状态未报告已连接（{error}），直接尝试打开 ANCS 会话");
            }
        }

        if self.stopped() {
            return Ok(RoundEnd::Stopped);
        }

        // ★ 链路刚通时 GATT 层还没就绪，这时候取服务会报 0x80070020。
        //   等下再取（真因是时序，不是进程占用 —— 这是 Rust 版新发现的）。
        if discovery::SETTLE_BEFORE_SERVICE > Duration::ZERO {
            sleep_unless_stopped(discovery::SETTLE_BEFORE_SERVICE, &self.stop);
        }

        self.emit(LinkState::Connected, Some("正在订阅 ANCS".to_string()));

        // 把 BLE 线程的通知转成 LinkEvent 报给上层
        let event_sink = Arc::clone(&self.sink);
        let notification_sink: NotificationSink = Arc::new(move |notification| {
            event_sink(LinkEvent::Notification(Box::new(notification)));
        });

        let session =
            session::AncsSession::open(&device, &device_id, &device_name, notification_sink)?;

        (self.sink)(LinkEvent::ActiveDevice(Some(DeviceInfo {
            id: device_id.clone(),
            name: device_name.clone(),
            connected: true,
        })));
        self.emit(LinkState::Subscribed, None);

        // ANCS 已稳定后再做一次可选读取，失败完全不影响通知链路。
        if let Some(level) = discovery::try_read_battery_level(&device) {
            info!("读取到设备电量：{level}%");
            (self.sink)(LinkEvent::BatteryLevel {
                device_id: device_id.clone(),
                level,
            });
        } else {
            info!("设备未提供可读取的标准 BLE 电量");
        }

        // 守着这条链路，直到断开或者被要求退出。
        //
        // ★ 同样不能只看 `ConnectionStatus`（见上）：容忍几秒抖动，
        //   连续多次都报未连接才认定为真断。
        let mut misses = 0u32;
        let mut last_events = session.event_count();
        let mut last_activity = std::time::Instant::now();
        let mut last_health_log = std::time::Instant::now();

        while !self.stopped() {
            std::thread::sleep(Duration::from_millis(1000));
            let events = session.event_count();
            if events != last_events {
                last_events = events;
                last_activity = std::time::Instant::now();
            }
            // Idle is not proof of failure. Reassert CCCDs without tearing down
            // a healthy connection; rebuild only if the operation fails.
            if last_activity.elapsed() >= Duration::from_secs(120) {
                info!("ANCS 静默两分钟，检查并刷新通知订阅");
                if let Err(error) = session.refresh_subscriptions() {
                    warn!("刷新订阅失败，重建会话：{error}");
                    break;
                }
                last_activity = std::time::Instant::now();
            }
            if last_health_log.elapsed() >= Duration::from_secs(60) {
                info!("链路监督仍运行：device_connected={} session_healthy={} notification_events={events}", discovery::is_connected(&device), session.is_connected());
                last_health_log = std::time::Instant::now();
            }

            // 设备级 ConnectionStatus 可能在 ANCS 的 GATT 会话静默关闭后仍报告 Connected。
            // 两层都健康才算链路可用，否则主动重建订阅。
            if discovery::is_connected(&device) && session.is_connected() {
                misses = 0;
            } else {
                misses += 1;

                if misses == 1 {
                    info!(
                        "链路健康检查未通过：device_connected={} gatt_session_active={}",
                        discovery::is_connected(&device),
                        session.is_connected()
                    );
                }

                if misses >= 5 {
                    info!("链路断开，准备重连");
                    break;
                }
            }
        }

        // 在可能耗时的清理前撤销就绪状态，避免 UI 在清理期间仍显示正常。
        if !self.stopped() {
            self.emit(
                LinkState::Reconnecting,
                Some("通知会话失效，正在重建连接".into()),
            );
        }
        (self.sink)(LinkEvent::ActiveDevice(None));
        session.close();

        Ok(if self.stopped() {
            RoundEnd::Stopped
        } else {
            RoundEnd::Disconnected
        })
    }
}

/// 检查蓝牙适配器能力。
///
/// ★ `IsPeripheralRoleSupported` 必须为真：ANCS 要求本机当 BLE 外设。
///   很多机器（尤其是老一点的适配器）只支持中心角色，那就是真的走不通。
fn check_adapter() -> Result<()> {
    let adapter = block_on_timeout(
        async { BluetoothAdapter::GetDefaultAsync()?.await },
        Duration::from_secs(8),
    )??;

    if !adapter.IsLowEnergySupported()? {
        return Err(BleError::AdapterUnsupported(
            "这台电脑的蓝牙不支持 BLE，SmsPop 无法工作".to_string(),
        ));
    }

    if !adapter.IsPeripheralRoleSupported()? {
        return Err(BleError::AdapterUnsupported(
            "这台电脑的蓝牙适配器不支持 BLE 外设角色，SmsPop 无法工作。\n\
             （ANCS 需要电脑扮演外设，很多适配器只支持中心角色）"
                .to_string(),
        ));
    }

    info!(
        "适配器就绪：地址={:X} 外设角色={} 广播卸载={}",
        adapter.BluetoothAddress()?,
        adapter.IsPeripheralRoleSupported()?,
        adapter.IsAdvertisementOffloadSupported()?
    );

    Ok(())
}
